//! Fossall client WASM — small interactive helpers for essay pages.
//!
//! Battery / range rough estimator used on `/rv`. Numbers are order-of-magnitude
//! only; not engineering design values.

use wasm_bindgen::prelude::*;

/// Rough battery energy density (pack-level, modern EV packs), Wh/kg.
const WH_PER_KG: f64 = 160.0;

/// Rough pack cost, USD per kWh (pack, not cell; volume mid-2020s planning band).
const USD_PER_KWH: f64 = 100.0;

/// Average consumption for a tall, boxy electric RV at modest speed, Wh/km.
const WH_PER_KM: f64 = 450.0;

/// Estimate usable pack energy (kWh), mass (kg), cost (USD), and range (km)
/// from a desired pack size in kWh.
///
/// Returns a newline-free summary string for DOM injection.
#[wasm_bindgen]
pub fn estimate_pack(kwh: f64) -> String {
    let kwh = kwh.clamp(20.0, 400.0);
    let mass_kg = (kwh * 1000.0) / WH_PER_KG;
    let cost_usd = kwh * USD_PER_KWH;
    let range_km = (kwh * 1000.0) / WH_PER_KM;
    let range_mi = range_km * 0.621371;

    format!(
        "{kwh:.0} kWh pack ≈ {mass_kg:.0} kg · ~${cost_usd:.0} · \
         ~{range_km:.0} km ({range_mi:.0} mi) at ~450 Wh/km (rough)"
    )
}

/// Default slider value for the essay widget (short-range product case).
#[wasm_bindgen]
pub fn default_kwh() -> f64 {
    90.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_is_sane() {
        let s = estimate_pack(90.0);
        assert!(s.contains("90 kWh"));
        assert!(s.contains("kg"));
        assert!(s.contains("km"));
    }

    #[test]
    fn default_is_short_range_product() {
        assert!((default_kwh() - 90.0).abs() < f64::EPSILON);
    }

    #[test]
    fn clamps_extremes() {
        let low = estimate_pack(1.0);
        assert!(low.contains("20 kWh"));
        let high = estimate_pack(999.0);
        assert!(high.contains("400 kWh"));
    }
}
