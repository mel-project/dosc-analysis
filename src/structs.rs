use anyhow::Context;
use chrono::NaiveDate;
use once_cell::sync::Lazy;
use smol_str::SmolStr;

/// A CPU model datapoint
#[derive(Clone, Debug)]
pub struct CpuModel {
    pub id: usize,
    pub name: SmolStr,
    pub raw_price: f64,
    pub sequential_perf: f64,
    pub date: NaiveDate,
    pub cores: f64,
    pub tdp: f64,
}

impl CpuModel {
    /// Parses a new CpuModel from a raw PassMark JSON item.
    pub fn from_json(json: &serde_json::Value) -> anyhow::Result<Self> {
        let id_str: SmolStr = serde_json::from_value(json.get("id").context("no id")?.clone())?;
        let id: usize = id_str.parse()?;
        let name: SmolStr = serde_json::from_value(json.get("name").context("no name")?.clone())?;
        // let raw_price: String =
        //     serde_json::from_value(json.get("price").context("no price")?.clone())?;
        // let raw_price: f64 = raw_price
        //     .replace("*", "")
        //     .replace("$", "")
        //     .replace(",", "")
        //     .parse()
        //     .context("cannot parse price")?;
        let perf: SmolStr =
            serde_json::from_value(json.get("thread").context("no thread")?.clone())?;
        let perf: f64 = perf.replace(",", "").parse().context("cannot parse perf")?;
        let parperf: SmolStr =
            serde_json::from_value(json.get("cpumark").context("no thread")?.clone())?;
        let parperf: f64 = parperf
            .replace(",", "")
            .parse()
            .context("cannot parse cpumark")?;
        let cores: SmolStr =
            serde_json::from_value(json.get("cores").context("no thread")?.clone())?;
        let cores: f64 = cores
            .replace(",", "")
            .parse()
            .context("cannot parse perf")?;
        let tdp: f64 =
            serde_json::from_value::<SmolStr>(json.get("tdp").context("no thread")?.clone())?
                .replace(",", "")
                .parse()
                .context("cannot parse perf")?;
        let date: SmolStr = serde_json::from_value(json.get("date").context("no date")?.clone())?;
        let date: NaiveDate = NaiveDate::parse_from_str(&format!("1 {}", date), "%d %b %Y")?;
        if !name.contains("Intel") {
            anyhow::bail!("not intel");
        }
        let price = model_price(name.clone());
        if price == f64::MAX {
            anyhow::bail!("wtf");
        }
        Ok(Self {
            id,
            name: name.clone(),
            raw_price: price,
            sequential_perf: perf,
            date,
            tdp,
            cores: parperf / perf,
        })
    }

    /// Daily cost estimate.
    pub fn daily_cost(&self, at_date: NaiveDate) -> f64 {
        let depreciation =
            self.price_at(at_date) - self.price_at(at_date + chrono::Duration::days(1));
        let kwh_per_day = self.tdp * 24.0 / 1000.0;
        let price = match ELECTRICITY_PRICES.binary_search_by_key(&at_date, |a| a.0) {
            Ok(n) => ELECTRICITY_PRICES[n].1,
            Err(n) => ELECTRICITY_PRICES[n].1,
        };
        depreciation + kwh_per_day * price
    }

    fn price_at(&self, at_date: NaiveDate) -> f64 {
        let age = at_date.signed_duration_since(self.date).num_days().abs() as f64 / 365.0;
        self.raw_price / 1.2f64.powf(age)
    }
}

// #[cached::proc_macro::cached]
fn model_price(model: SmolStr) -> f64 {
    let (probable_model, price) = MODEL_TO_PRICE
        .iter()
        .min_by_key(|s| {
            let model = model.clone();
            model
                .split_whitespace()
                .map(|frag| levenshtein::levenshtein(&s.0, frag))
                .min()
                .unwrap()
        })
        .unwrap();
    eprintln!("model {} has closest match {}", model, probable_model);
    *price
}

/// Model name to price, read off of the ARK csv
static MODEL_TO_PRICE: Lazy<Vec<(SmolStr, f64)>> = Lazy::new(|| {
    let file = String::from_utf8_lossy(include_bytes!("output.csv"));
    file.lines()
        .filter_map(|line| {
            let split = line.split(';').collect::<Vec<_>>();
            let name = split.get(2).map(|a| a.to_string())?;
            let price: f64 = split.get(5).map(|a| a.parse().unwrap_or(f64::MAX))?;
            Some((name.into(), price))
        })
        .collect()
});

/// Electricity prices
static ELECTRICITY_PRICES: Lazy<Vec<(NaiveDate, f64)>> = Lazy::new(|| {
    let file = include_str!("APU000072610.csv");
    file.lines()
        .filter_map(|line| {
            let split = line.split(',').collect::<Vec<_>>();
            let date: NaiveDate = NaiveDate::parse_from_str(split.get(0)?, "%Y-%m-%d").ok()?;
            let price: f64 = split.get(1)?.parse().ok()?;
            Some((date, price))
        })
        .collect()
});
