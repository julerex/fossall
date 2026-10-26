-- Simulated shop: catalog, X-login users, carts, and fake orders.
-- Apply against database `fossall` on cluster `postgreputest`.
-- See docs/SHOP.md and docs/DATABASE.md.

CREATE SCHEMA IF NOT EXISTS shop;

CREATE TABLE IF NOT EXISTS shop.users (
    id BIGSERIAL PRIMARY KEY,
    x_user_id TEXT NOT NULL UNIQUE,
    username TEXT NOT NULL,
    display_name TEXT NOT NULL,
    avatar_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS shop.products (
    id BIGSERIAL PRIMARY KEY,
    sku TEXT NOT NULL UNIQUE
        CHECK (
            char_length(sku) BETWEEN 2 AND 40
            AND sku ~ '^[a-z0-9-]+$'
        ),
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    price_cents INTEGER NOT NULL CHECK (price_cents > 0)
);

CREATE TABLE IF NOT EXISTS shop.cart_items (
    user_id BIGINT NOT NULL REFERENCES shop.users (id) ON DELETE CASCADE,
    product_id BIGINT NOT NULL REFERENCES shop.products (id),
    quantity INTEGER NOT NULL CHECK (quantity >= 1 AND quantity <= 99),
    PRIMARY KEY (user_id, product_id)
);

CREATE TABLE IF NOT EXISTS shop.orders (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES shop.users (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    status TEXT NOT NULL DEFAULT 'simulated' CHECK (status = 'simulated'),
    total_cents INTEGER NOT NULL CHECK (total_cents >= 0)
);

CREATE TABLE IF NOT EXISTS shop.order_items (
    id BIGSERIAL PRIMARY KEY,
    order_id BIGINT NOT NULL REFERENCES shop.orders (id) ON DELETE CASCADE,
    sku TEXT NOT NULL,
    name TEXT NOT NULL,
    quantity INTEGER NOT NULL CHECK (quantity >= 1 AND quantity <= 99),
    unit_price_cents INTEGER NOT NULL CHECK (unit_price_cents > 0)
);

CREATE INDEX IF NOT EXISTS orders_user_created_idx
    ON shop.orders (user_id, created_at DESC);

INSERT INTO shop.products (sku, name, description, price_cents)
VALUES
    (
        'sticker-sheet',
        'Fossall sticker sheet',
        'Six vinyl stickers of the Fossall mark, the container RV, and a five-letter grid.',
        500
    ),
    (
        'rv-poster',
        'Container RV poster',
        'A large print of the shipping-container electric RV sketch from the essay.',
        1800
    ),
    (
        'globe-print',
        'Deep-time globe print',
        'Continents at 1.8 billion years ago, drawn from the Earth globe.',
        2400
    ),
    (
        'five-letter-chart',
        'Five-letter chart',
        'ENABLE five-letter words on one dense green wall chart.',
        1200
    ),
    (
        'solar-pamphlet',
        'Solar-kit pamphlet',
        'A short pamphlet on rooftop solar yield for a parked container RV.',
        800
    ),
    (
        'supporter-pin',
        'Supporter pin',
        'A small enamel pin. Simulated enamel. Simulated pin.',
        600
    )
ON CONFLICT (sku) DO UPDATE
SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    price_cents = EXCLUDED.price_cents;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'fossall') THEN
        GRANT USAGE ON SCHEMA shop TO fossall;
        GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE
            shop.users,
            shop.products,
            shop.cart_items,
            shop.orders,
            shop.order_items
        TO fossall;
        GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA shop TO fossall;
    END IF;
END
$$;
