//! Douglas-Peucker simplification for GeoJSON coordinate rings.
//! Coordinates are [lon, lat] in degrees.

/// Perpendicular distance from `p` to segment `a`→`b`.
fn perp_dist(p: &[f64], a: &[f64], b: &[f64]) -> f64 {
    let (x, y) = (p[0], p[1]);
    let (x1, y1) = (a[0], a[1]);
    let (x2, y2) = (b[0], b[1]);
    let dx = x2 - x1;
    let dy = y2 - y1;
    if dx == 0.0 && dy == 0.0 {
        return ((x - x1).powi(2) + (y - y1).powi(2)).sqrt();
    }
    let t = ((x - x1) * dx + (y - y1) * dy) / (dx * dx + dy * dy);
    let t = t.clamp(0.0, 1.0);
    let px = x1 + t * dx;
    let py = y1 + t * dy;
    ((x - px).powi(2) + (y - py).powi(2)).sqrt()
}

/// Ramer–Douglas–Peucker. Keeps first and last points. `epsilon` is in degrees.
pub fn rdp(points: &[[f64; 2]], epsilon: f64) -> Vec<[f64; 2]> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut idx = 0;
    let mut max_d = 0.0;
    let last = points.len() - 1;
    for i in 1..last {
        let d = perp_dist(&points[i], &points[0], &points[last]);
        if d > max_d {
            idx = i;
            max_d = d;
        }
    }
    if max_d > epsilon {
        let mut left = rdp(&points[..=idx], epsilon);
        let right = rdp(&points[idx..], epsilon);
        left.pop();
        left.extend(right);
        left
    } else {
        vec![points[0], points[last]]
    }
}

pub fn simplify_ring(ring: &[[f64; 2]], epsilon: f64) -> Vec<[f64; 2]> {
    if ring.len() < 4 {
        return ring.to_vec();
    }
    let closed = ring.first() == ring.last();
    let body = if closed {
        &ring[..ring.len() - 1]
    } else {
        ring
    };
    let mut out = rdp(body, epsilon);
    if closed {
        if let Some(first) = out.first().copied() {
            if out.last() != Some(&first) {
                out.push(first);
            }
        }
        if out.len() < 4 {
            return ring.to_vec();
        }
    }
    out
}

/// Approximate polygon area in square degrees (absolute shoelace).
pub fn ring_area(ring: &[[f64; 2]]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..ring.len() - 1 {
        sum += ring[i][0] * ring[i + 1][1] - ring[i + 1][0] * ring[i][1];
    }
    (sum / 2.0).abs()
}

fn coords_to_ring(coords: &[serde_json::Value]) -> Option<Vec<[f64; 2]>> {
    let mut ring = Vec::with_capacity(coords.len());
    for pt in coords {
        let arr = pt.as_array()?;
        if arr.len() < 2 {
            return None;
        }
        ring.push([arr[0].as_f64()?, arr[1].as_f64()?]);
    }
    Some(ring)
}

fn ring_to_json(ring: &[[f64; 2]]) -> serde_json::Value {
    serde_json::Value::Array(
        ring.iter()
            .map(|p| serde_json::json!([p[0], p[1]]))
            .collect(),
    )
}

/// Simplify a GeoJSON geometry. Returns None if the feature is too small.
pub fn simplify_geometry(
    geom: &serde_json::Value,
    epsilon: f64,
    min_area: f64,
) -> Option<serde_json::Value> {
    let gtype = geom.get("type")?.as_str()?;
    let coords = geom.get("coordinates")?;
    match gtype {
        "Polygon" => {
            let rings = coords.as_array()?;
            let outer = coords_to_ring(rings.first()?.as_array()?)?;
            if ring_area(&outer) < min_area {
                return None;
            }
            let mut out_rings = vec![ring_to_json(&simplify_ring(&outer, epsilon))];
            for hole in rings.iter().skip(1) {
                let Some(h) = coords_to_ring(hole.as_array()?) else {
                    continue;
                };
                if ring_area(&h) < min_area * 0.25 {
                    continue;
                }
                out_rings.push(ring_to_json(&simplify_ring(&h, epsilon)));
            }
            Some(serde_json::json!({
                "type": "Polygon",
                "coordinates": out_rings
            }))
        }
        "MultiPolygon" => {
            let polys = coords.as_array()?;
            let mut kept = Vec::new();
            for poly in polys {
                let fake = serde_json::json!({"type": "Polygon", "coordinates": poly});
                if let Some(s) = simplify_geometry(&fake, epsilon, min_area) {
                    kept.push(s.get("coordinates")?.clone());
                }
            }
            if kept.is_empty() {
                return None;
            }
            if kept.len() == 1 {
                return Some(serde_json::json!({
                    "type": "Polygon",
                    "coordinates": kept[0]
                }));
            }
            Some(serde_json::json!({
                "type": "MultiPolygon",
                "coordinates": kept
            }))
        }
        "LineString" => {
            let ring = coords_to_ring(coords.as_array()?)?;
            if ring.len() < 2 {
                return None;
            }
            let simple = simplify_ring(&ring, epsilon);
            if simple.len() < 2 {
                return None;
            }
            Some(serde_json::json!({
                "type": "LineString",
                "coordinates": ring_to_json(&simple)
            }))
        }
        _ => None,
    }
}

pub fn geometry_bbox(geom: &serde_json::Value) -> Option<(f64, f64, f64, f64)> {
    let mut west = f64::MAX;
    let mut east = f64::MIN;
    let mut south = f64::MAX;
    let mut north = f64::MIN;
    fn walk(v: &serde_json::Value, w: &mut f64, e: &mut f64, s: &mut f64, n: &mut f64) {
        if let Some(arr) = v.as_array() {
            if arr.len() >= 2 && arr[0].is_number() && arr[1].is_number() {
                let x = arr[0].as_f64().unwrap_or(0.0);
                let y = arr[1].as_f64().unwrap_or(0.0);
                *w = w.min(x);
                *e = e.max(x);
                *s = s.min(y);
                *n = n.max(y);
            } else {
                for child in arr {
                    walk(child, w, e, s, n);
                }
            }
        }
    }
    walk(
        geom.get("coordinates")?,
        &mut west,
        &mut east,
        &mut south,
        &mut north,
    );
    if west == f64::MAX {
        None
    } else {
        Some((west, south, east, north))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rdp_keeps_endpoints_of_a_line() {
        let pts = [[0.0, 0.0], [1.0, 0.05], [2.0, 0.0]];
        let out = rdp(&pts, 0.1);
        assert_eq!(out.first(), Some(&[0.0, 0.0]));
        assert_eq!(out.last(), Some(&[2.0, 0.0]));
        assert!(out.len() <= 3);
    }

    #[test]
    fn rdp_drops_colinear_middle() {
        let pts = [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]];
        let out = rdp(&pts, 0.01);
        assert_eq!(out, vec![[0.0, 0.0], [2.0, 0.0]]);
    }

    #[test]
    fn ring_area_unit_square() {
        let ring = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]];
        assert!((ring_area(&ring) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn simplify_drops_tiny_polygon() {
        let geom = serde_json::json!({
            "type": "Polygon",
            "coordinates": [[[0.0, 0.0], [0.01, 0.0], [0.01, 0.01], [0.0, 0.01], [0.0, 0.0]]]
        });
        assert!(simplify_geometry(&geom, 0.2, 0.05).is_none());
    }

    #[test]
    fn simplify_keeps_large_polygon() {
        let geom = serde_json::json!({
            "type": "Polygon",
            "coordinates": [[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0], [0.0, 0.0]]]
        });
        let out = simplify_geometry(&geom, 0.2, 0.05).unwrap();
        assert_eq!(out["type"], "Polygon");
        let bbox = geometry_bbox(&out).unwrap();
        assert!((bbox.0 - 0.0).abs() < 1e-6);
        assert!((bbox.2 - 10.0).abs() < 1e-6);
    }
}
