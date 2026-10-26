//! Simulated shop: catalog, cart, fake checkout, and order history.

use axum::extract::{Form, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use maud::{html, Markup};
use serde::Deserialize;
use sqlx::{PgPool, Row};

use crate::auth::{self, ShopUser};
use crate::layout;
use crate::AppState;

const MIN_QUANTITY: i32 = 1;
const MAX_QUANTITY: i32 = 99;

#[derive(Debug, Clone)]
struct Product {
    id: i64,
    sku: String,
    name: String,
    description: String,
    price_cents: i32,
}

#[derive(Debug, Clone)]
struct CartLine {
    product: Product,
    quantity: i32,
}

#[derive(Debug, Clone)]
struct OrderSummary {
    id: i64,
    created_at: String,
    total_cents: i32,
}

#[derive(Debug, Clone)]
struct OrderLine {
    sku: String,
    name: String,
    quantity: i32,
    unit_price_cents: i32,
}

#[derive(Debug, Clone)]
struct OrderDetail {
    summary: OrderSummary,
    items: Vec<OrderLine>,
}

struct ShopFrame {
    user: Option<ShopUser>,
    cart_count: i64,
    login_configured: bool,
}

#[derive(Debug, Deserialize)]
pub struct CartForm {
    pub sku: String,
    pub quantity: Option<i32>,
    pub action: Option<String>,
}

/// `GET /shop` — public catalog.
pub async fn catalog(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(pool) = state.db.as_ref() else {
        return unavailable();
    };
    let frame = load_frame(&state, pool, &headers).await;
    match list_products(pool).await {
        Ok(products) => shop_ok("Shop", catalog_page(&frame, &products)),
        Err(err) => {
            tracing::error!(?err, "shop catalog query failed");
            shop_fail(
                "Shop",
                &frame,
                "Could not read the catalog from Postgres. Try again shortly.",
            )
        }
    }
}

/// `GET /shop/{sku}` — public product.
pub async fn product(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(sku): Path<String>,
) -> Response {
    let Some(pool) = state.db.as_ref() else {
        return unavailable();
    };
    let Some(sku) = sanitize_sku(&sku) else {
        return not_found();
    };
    let frame = load_frame(&state, pool, &headers).await;
    match product_by_sku(pool, &sku).await {
        Ok(Some(product)) => shop_ok(&product.name, product_page(&frame, &product)),
        Ok(None) => not_found(),
        Err(err) => {
            tracing::error!(?err, "shop product query failed");
            shop_fail(
                "Shop",
                &frame,
                "Could not read that product from Postgres. Try again shortly.",
            )
        }
    }
}

/// `GET /shop/cart` — signed-in cart.
pub async fn cart(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(pool) = state.db.as_ref() else {
        return unavailable();
    };
    let frame = load_frame(&state, pool, &headers).await;
    let Some(user) = frame.user.as_ref() else {
        return require_login(&state, "/shop/cart");
    };
    match list_cart(pool, user.id).await {
        Ok(lines) => shop_ok("Cart", cart_page(&frame, &lines)),
        Err(err) => {
            tracing::error!(?err, "shop cart query failed");
            shop_fail(
                "Cart",
                &frame,
                "Could not read the cart from Postgres. Try again shortly.",
            )
        }
    }
}

/// `POST /shop/cart` — set quantity, add, or remove, then redirect to the cart.
pub async fn update_cart(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<CartForm>,
) -> Response {
    let Some(pool) = state.db.as_ref() else {
        return unavailable();
    };
    let frame = load_frame(&state, pool, &headers).await;
    let Some(sku) = sanitize_sku(&form.sku) else {
        return not_found();
    };
    let Some(user) = frame.user.as_ref() else {
        return require_login(&state, &format!("/shop/{sku}"));
    };
    let product = match product_by_sku(pool, &sku).await {
        Ok(Some(product)) => product,
        Ok(None) => return not_found(),
        Err(err) => {
            tracing::error!(?err, "shop cart product lookup failed");
            return shop_fail(
                "Cart",
                &frame,
                "Could not update the cart. Try again shortly.",
            );
        }
    };

    let action = form.action.as_deref().unwrap_or("set");
    let result = match action {
        "remove" => remove_cart_line(pool, user.id, product.id).await,
        "add" => {
            let quantity = clamp_quantity(form.quantity.unwrap_or(1)).unwrap_or(MIN_QUANTITY);
            add_cart_line(pool, user.id, product.id, quantity).await
        }
        _ => match clamp_quantity(form.quantity.unwrap_or(0)) {
            Some(quantity) => set_cart_line(pool, user.id, product.id, quantity).await,
            None => remove_cart_line(pool, user.id, product.id).await,
        },
    };

    match result {
        Ok(()) => Redirect::to("/shop/cart").into_response(),
        Err(err) => {
            tracing::error!(?err, "shop cart write failed");
            shop_fail(
                "Cart",
                &frame,
                "Could not update the cart. Try again shortly.",
            )
        }
    }
}

/// `POST /shop/checkout` — write a simulated order and clear the cart.
pub async fn checkout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(pool) = state.db.as_ref() else {
        return unavailable();
    };
    let frame = load_frame(&state, pool, &headers).await;
    let Some(user) = frame.user.as_ref() else {
        return require_login(&state, "/shop/cart");
    };
    match place_order(pool, user.id).await {
        Ok(None) => Redirect::to("/shop/cart").into_response(),
        Ok(Some(order_id)) => Redirect::to(&format!("/shop/orders/{order_id}")).into_response(),
        Err(err) => {
            tracing::error!(?err, "shop checkout failed");
            shop_fail(
                "Checkout",
                &frame,
                "Could not place the simulated order. Try again shortly.",
            )
        }
    }
}

/// `GET /shop/orders` — that user's history.
pub async fn orders(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(pool) = state.db.as_ref() else {
        return unavailable();
    };
    let frame = load_frame(&state, pool, &headers).await;
    let Some(user) = frame.user.as_ref() else {
        return require_login(&state, "/shop/orders");
    };
    match list_orders(pool, user.id).await {
        Ok(orders) => shop_ok("Orders", orders_page(&frame, &orders)),
        Err(err) => {
            tracing::error!(?err, "shop orders query failed");
            shop_fail(
                "Orders",
                &frame,
                "Could not read orders from Postgres. Try again shortly.",
            )
        }
    }
}

/// `GET /shop/orders/{id}` — receipt for the owner.
pub async fn order(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Response {
    if id <= 0 {
        return not_found();
    }
    let Some(pool) = state.db.as_ref() else {
        return unavailable();
    };
    let frame = load_frame(&state, pool, &headers).await;
    let Some(user) = frame.user.as_ref() else {
        return require_login(&state, &format!("/shop/orders/{id}"));
    };
    match load_order(pool, user.id, id).await {
        Ok(Some(detail)) => shop_ok("Receipt", receipt_page(&frame, &detail)),
        Ok(None) => not_found(),
        Err(err) => {
            tracing::error!(?err, "shop order query failed");
            shop_fail(
                "Receipt",
                &frame,
                "Could not read that order from Postgres. Try again shortly.",
            )
        }
    }
}

pub fn format_usd(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let cents = cents.unsigned_abs();
    format!("{sign}${}.{:02}", cents / 100, cents % 100)
}

pub fn line_cents(unit_price_cents: i32, quantity: i32) -> i64 {
    i64::from(unit_price_cents) * i64::from(quantity)
}

pub fn clamp_quantity(raw: i32) -> Option<i32> {
    if raw < MIN_QUANTITY {
        None
    } else {
        Some(raw.min(MAX_QUANTITY))
    }
}

pub fn sanitize_sku(raw: &str) -> Option<String> {
    if raw.len() < 2 || raw.len() > 40 {
        return None;
    }
    if !raw
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    {
        return None;
    }
    Some(raw.to_ascii_lowercase())
}

async fn load_frame(state: &AppState, pool: &PgPool, headers: &HeaderMap) -> ShopFrame {
    let user = auth::session_user(pool, &state.shop, headers).await;
    let cart_count = match user.as_ref() {
        Some(user) => cart_count(pool, user.id).await.unwrap_or(0),
        None => 0,
    };
    ShopFrame {
        user,
        cart_count,
        login_configured: state.shop.login_configured(),
    }
}

fn product_from_row(row: &sqlx::postgres::PgRow) -> Product {
    Product {
        id: row.get("id"),
        sku: row.get("sku"),
        name: row.get("name"),
        description: row.get("description"),
        price_cents: row.get("price_cents"),
    }
}

async fn list_products(pool: &PgPool) -> Result<Vec<Product>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, sku, name, description, price_cents
         FROM shop.products
         ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(product_from_row).collect())
}

async fn product_by_sku(pool: &PgPool, sku: &str) -> Result<Option<Product>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, sku, name, description, price_cents
         FROM shop.products
         WHERE sku = $1",
    )
    .bind(sku)
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(product_from_row))
}

async fn cart_count(pool: &PgPool, user_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COALESCE(SUM(quantity), 0)::bigint
         FROM shop.cart_items
         WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
}

async fn list_cart(pool: &PgPool, user_id: i64) -> Result<Vec<CartLine>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT p.id, p.sku, p.name, p.description, p.price_cents, c.quantity
         FROM shop.cart_items c
         JOIN shop.products p ON p.id = c.product_id
         WHERE c.user_id = $1
         ORDER BY p.id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|row| CartLine {
            product: product_from_row(row),
            quantity: row.get("quantity"),
        })
        .collect())
}

async fn add_cart_line(
    pool: &PgPool,
    user_id: i64,
    product_id: i64,
    quantity: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO shop.cart_items (user_id, product_id, quantity)
         VALUES ($1, $2, $3)
         ON CONFLICT (user_id, product_id) DO UPDATE
         SET quantity = LEAST(99, shop.cart_items.quantity + EXCLUDED.quantity)",
    )
    .bind(user_id)
    .bind(product_id)
    .bind(quantity)
    .execute(pool)
    .await?;
    Ok(())
}

async fn set_cart_line(
    pool: &PgPool,
    user_id: i64,
    product_id: i64,
    quantity: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO shop.cart_items (user_id, product_id, quantity)
         VALUES ($1, $2, $3)
         ON CONFLICT (user_id, product_id) DO UPDATE
         SET quantity = EXCLUDED.quantity",
    )
    .bind(user_id)
    .bind(product_id)
    .bind(quantity)
    .execute(pool)
    .await?;
    Ok(())
}

async fn remove_cart_line(pool: &PgPool, user_id: i64, product_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM shop.cart_items
         WHERE user_id = $1 AND product_id = $2",
    )
    .bind(user_id)
    .bind(product_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn place_order(pool: &PgPool, user_id: i64) -> Result<Option<i64>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let rows = sqlx::query(
        "SELECT p.sku, p.name, p.price_cents, c.quantity
         FROM shop.cart_items c
         JOIN shop.products p ON p.id = c.product_id
         WHERE c.user_id = $1
         FOR UPDATE OF c",
    )
    .bind(user_id)
    .fetch_all(&mut *tx)
    .await?;
    if rows.is_empty() {
        return Ok(None);
    }

    let mut total: i64 = 0;
    let mut items: Vec<(String, String, i32, i32)> = Vec::with_capacity(rows.len());
    for row in &rows {
        let quantity: i32 = row.get("quantity");
        let unit_price_cents: i32 = row.get("price_cents");
        total = total.saturating_add(line_cents(unit_price_cents, quantity));
        items.push((row.get("sku"), row.get("name"), quantity, unit_price_cents));
    }
    let Ok(total_cents) = i32::try_from(total) else {
        return Err(sqlx::Error::Protocol("order total does not fit".into()));
    };

    let order_id: i64 = sqlx::query_scalar(
        "INSERT INTO shop.orders (user_id, status, total_cents)
         VALUES ($1, 'simulated', $2)
         RETURNING id",
    )
    .bind(user_id)
    .bind(total_cents)
    .fetch_one(&mut *tx)
    .await?;

    for (sku, name, quantity, unit_price_cents) in items {
        sqlx::query(
            "INSERT INTO shop.order_items (order_id, sku, name, quantity, unit_price_cents)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(order_id)
        .bind(sku)
        .bind(name)
        .bind(quantity)
        .bind(unit_price_cents)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("DELETE FROM shop.cart_items WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Some(order_id))
}

async fn list_orders(pool: &PgPool, user_id: i64) -> Result<Vec<OrderSummary>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, total_cents,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI') || ' UTC' AS created_at
         FROM shop.orders
         WHERE user_id = $1
         ORDER BY created_at DESC, id DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|row| OrderSummary {
            id: row.get("id"),
            created_at: row.get("created_at"),
            total_cents: row.get("total_cents"),
        })
        .collect())
}

async fn load_order(
    pool: &PgPool,
    user_id: i64,
    order_id: i64,
) -> Result<Option<OrderDetail>, sqlx::Error> {
    let Some(header) = sqlx::query(
        "SELECT id, total_cents,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI') || ' UTC' AS created_at
         FROM shop.orders
         WHERE id = $1 AND user_id = $2",
    )
    .bind(order_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    let item_rows = sqlx::query(
        "SELECT sku, name, quantity, unit_price_cents
         FROM shop.order_items
         WHERE order_id = $1
         ORDER BY id",
    )
    .bind(order_id)
    .fetch_all(pool)
    .await?;

    Ok(Some(OrderDetail {
        summary: OrderSummary {
            id: header.get("id"),
            created_at: header.get("created_at"),
            total_cents: header.get("total_cents"),
        },
        items: item_rows
            .iter()
            .map(|row| OrderLine {
                sku: row.get("sku"),
                name: row.get("name"),
                quantity: row.get("quantity"),
                unit_price_cents: row.get("unit_price_cents"),
            })
            .collect(),
    }))
}

fn shop_ok(title: &str, body: Markup) -> Response {
    (StatusCode::OK, layout::page(title, body)).into_response()
}

fn shop_fail(title: &str, frame: &ShopFrame, message: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        layout::page(title, error_page(frame, title, message)),
    )
        .into_response()
}

fn unavailable() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        layout::page(
            "Shop",
            html! {
                section class="shop" {
                    (banner())
                    header class="essay-header" {
                        p class="eyebrow" { "Simulated shop" }
                        h1 { "Shop" }
                    }
                    p class="words-error" {
                        "The shop is not connected in this process. "
                        "Set DATABASE_URL (see docs/DATABASE.md and docs/SHOP.md) and restart."
                    }
                }
            },
        ),
    )
        .into_response()
}

fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        layout::page(
            "Not found",
            html! {
                section class="shop" {
                    (banner())
                    header class="essay-header" {
                        p class="eyebrow" { "Simulated shop" }
                        h1 { "Not found" }
                    }
                    p { "That shop page does not exist." }
                    p {
                        a class="btn secondary" href="/shop" { "Back to the catalog" }
                    }
                }
            },
        ),
    )
        .into_response()
}

fn require_login(state: &AppState, next: &str) -> Response {
    if state.shop.login_configured() {
        return Redirect::to(&auth::sign_in_href(next)).into_response();
    }
    (
        StatusCode::OK,
        layout::page(
            "Sign in",
            html! {
                section class="shop" {
                    (banner())
                    header class="essay-header" {
                        p class="eyebrow" { "Simulated shop" }
                        h1 { "Sign in with X" }
                    }
                    p class="words-error" {
                        "Sign-in is not configured on this process. "
                        "Set X_CLIENT_ID, X_CLIENT_SECRET, and SHOP_SESSION_SECRET "
                        "(see docs/SHOP.md)."
                    }
                    p {
                        a class="btn secondary" href="/shop" { "Back to the catalog" }
                    }
                }
            },
        ),
    )
        .into_response()
}

fn banner() -> Markup {
    html! {
        p class="shop-banner" role="status" {
            strong { "Simulation." }
            " Nothing is charged or shipped."
        }
    }
}

fn account_bar(frame: &ShopFrame, here: &str) -> Markup {
    html! {
        div class="shop-bar" {
            nav class="shop-links" {
                a href="/shop" { "Catalog" }
                a href="/shop/cart" { "Cart (" (frame.cart_count) ")" }
                a href="/shop/orders" { "Orders" }
            }
            div class="shop-account" {
                @if let Some(user) = frame.user.as_ref() {
                    @if let Some(url) = user.avatar_url.as_deref() {
                        img class="shop-avatar" src=(url) alt="" width="28" height="28";
                    }
                    span class="shop-handle" { "@" (user.username) }
                    form class="shop-logout" method="post" action="/auth/logout" hx-boost="false" {
                        button class="btn secondary" type="submit" { "Sign out" }
                    }
                } @else if frame.login_configured {
                    a class="btn" href=(auth::sign_in_href(here)) hx-boost="false" {
                        "Sign in with X"
                    }
                } @else {
                    span class="shop-unconfigured" { "Sign-in is not configured." }
                }
            }
        }
    }
}

fn catalog_page(frame: &ShopFrame, products: &[Product]) -> Markup {
    html! {
        section class="shop" {
            (banner())
            header class="essay-header" {
                p class="eyebrow" { "Simulated shop · X login" }
                h1 { "Fossall shop" }
                p class="lede" {
                    "A fake storefront for Fossall merch. Browse freely. "
                    "A cart and a receipt wait behind an X account. Nobody is charged."
                }
            }
            (account_bar(frame, "/shop"))
            @if products.is_empty() {
                p class="words-error" {
                    "The catalog is empty. Apply db/002_shop.sql (see docs/SHOP.md)."
                }
            } @else {
                ul class="shop-grid" {
                    @for product in products {
                        li {
                            a class="shop-card" href=(format!("/shop/{}", product.sku)) {
                                (art(&product.sku))
                                h2 { (product.name) }
                                p { (product.description) }
                                p class="shop-price tabular" { (format_usd(i64::from(product.price_cents))) }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn product_page(frame: &ShopFrame, product: &Product) -> Markup {
    let here = format!("/shop/{}", product.sku);
    html! {
        section class="shop" {
            (banner())
            header class="essay-header" {
                p class="eyebrow" { "Simulated shop" }
                h1 { (product.name) }
                p class="lede" { (product.description) }
            }
            (account_bar(frame, &here))
            div class="shop-product" {
                (art(&product.sku))
                p class="shop-price tabular" { (format_usd(i64::from(product.price_cents))) }
                @if frame.user.is_some() {
                    form method="post" action="/shop/cart" hx-boost="false" {
                        input type="hidden" name="sku" value=(product.sku);
                        input type="hidden" name="action" value="add";
                        input type="hidden" name="quantity" value="1";
                        button class="btn" type="submit" { "Add to cart" }
                    }
                } @else if frame.login_configured {
                    a class="btn" href=(auth::sign_in_href(&here)) hx-boost="false" {
                        "Sign in with X to add this"
                    }
                } @else {
                    p class="shop-unconfigured" { "Sign-in is not configured, so the cart stays closed." }
                }
                p {
                    a href="/shop" { "Back to the catalog" }
                }
            }
        }
    }
}

fn cart_page(frame: &ShopFrame, lines: &[CartLine]) -> Markup {
    let total: i64 = lines
        .iter()
        .map(|line| line_cents(line.product.price_cents, line.quantity))
        .sum();
    html! {
        section class="shop" {
            (banner())
            header class="essay-header" {
                p class="eyebrow" { "Simulated shop" }
                h1 { "Cart" }
            }
            (account_bar(frame, "/shop/cart"))
            @if lines.is_empty() {
                p { "The cart is empty." }
                p {
                    a class="btn secondary" href="/shop" { "Browse the catalog" }
                }
            } @else {
                ul class="shop-lines" {
                    @for line in lines {
                        li class="shop-line" {
                            div {
                                a href=(format!("/shop/{}", line.product.sku)) { (line.product.name) }
                                p class="shop-price tabular" {
                                    (format_usd(line_cents(line.product.price_cents, line.quantity)))
                                }
                            }
                            form class="shop-qty" method="post" action="/shop/cart" hx-boost="false" {
                                input type="hidden" name="sku" value=(line.product.sku);
                                input type="hidden" name="action" value="set";
                                label {
                                    "Qty "
                                    input type="number" name="quantity" min="1" max="99"
                                        value=(line.quantity) required;
                                }
                                button class="btn secondary" type="submit" { "Update" }
                            }
                            form method="post" action="/shop/cart" hx-boost="false" {
                                input type="hidden" name="sku" value=(line.product.sku);
                                input type="hidden" name="action" value="remove";
                                button class="btn secondary" type="submit" { "Remove" }
                            }
                        }
                    }
                }
                p class="shop-total tabular" {
                    "Total "
                    (format_usd(total))
                }
                form method="post" action="/shop/checkout" hx-boost="false" {
                    button class="btn" type="submit" { "Place simulated order" }
                }
            }
        }
    }
}

fn orders_page(frame: &ShopFrame, orders: &[OrderSummary]) -> Markup {
    html! {
        section class="shop" {
            (banner())
            header class="essay-header" {
                p class="eyebrow" { "Simulated shop" }
                h1 { "Orders" }
            }
            (account_bar(frame, "/shop/orders"))
            @if orders.is_empty() {
                p { "No simulated orders yet." }
                p {
                    a class="btn secondary" href="/shop" { "Browse the catalog" }
                }
            } @else {
                ul class="shop-orders" {
                    @for order in orders {
                        li {
                            a href=(format!("/shop/orders/{}", order.id)) {
                                span class="tabular" { "#" (order.id) }
                                " · "
                                (order.created_at)
                                " · "
                                span class="tabular" { (format_usd(i64::from(order.total_cents))) }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn receipt_page(frame: &ShopFrame, detail: &OrderDetail) -> Markup {
    html! {
        section class="shop" {
            (banner())
            header class="essay-header" {
                p class="eyebrow" { "Simulated receipt" }
                h1 { "Order #" (detail.summary.id) }
                p class="lede" {
                    (detail.summary.created_at)
                    " · status simulated · nothing was charged"
                }
            }
            (account_bar(frame, &format!("/shop/orders/{}", detail.summary.id)))
            ul class="shop-lines" {
                @for item in &detail.items {
                    li class="shop-line" {
                        div {
                            strong { (item.name) }
                            p class="shop-muted" { (item.sku) }
                        }
                        p class="tabular" { (item.quantity) " × " (format_usd(i64::from(item.unit_price_cents))) }
                        p class="shop-price tabular" {
                            (format_usd(line_cents(item.unit_price_cents, item.quantity)))
                        }
                    }
                }
            }
            p class="shop-total tabular" {
                "Total "
                (format_usd(i64::from(detail.summary.total_cents)))
            }
            p {
                a href="/shop/orders" { "All orders" }
                " · "
                a href="/shop" { "Catalog" }
            }
        }
    }
}

fn error_page(frame: &ShopFrame, title: &str, message: &str) -> Markup {
    html! {
        section class="shop" {
            (banner())
            header class="essay-header" {
                p class="eyebrow" { "Simulated shop" }
                h1 { (title) }
            }
            (account_bar(frame, "/shop"))
            p class="words-error" { (message) }
        }
    }
}

fn art(sku: &str) -> Markup {
    html! {
        div class=(format!("shop-art shop-art-{sku}")) aria-hidden="true" {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_usd_splits_cents() {
        assert_eq!(format_usd(0), "$0.00");
        assert_eq!(format_usd(500), "$5.00");
        assert_eq!(format_usd(1800), "$18.00");
        assert_eq!(format_usd(99), "$0.99");
        assert_eq!(format_usd(-50), "-$0.50");
    }

    #[test]
    fn line_cents_multiplies_without_i32_overflow() {
        assert_eq!(line_cents(2400, 2), 4800);
        assert_eq!(line_cents(2400, 99), 237_600);
    }

    #[test]
    fn clamp_quantity_bounds_one_to_ninety_nine() {
        assert_eq!(clamp_quantity(0), None);
        assert_eq!(clamp_quantity(-3), None);
        assert_eq!(clamp_quantity(1), Some(1));
        assert_eq!(clamp_quantity(50), Some(50));
        assert_eq!(clamp_quantity(99), Some(99));
        assert_eq!(clamp_quantity(100), Some(99));
    }

    #[test]
    fn sanitize_sku_keeps_catalog_keys() {
        assert_eq!(sanitize_sku("sticker-sheet"), Some("sticker-sheet".into()));
        assert_eq!(sanitize_sku("RV-Poster"), Some("rv-poster".into()));
        assert_eq!(sanitize_sku("x"), None);
        assert_eq!(sanitize_sku("../etc"), None);
    }
}
