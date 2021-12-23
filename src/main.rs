use std::collections::BTreeMap;

use chrono::NaiveDate;
use rayon::{
    iter::{IntoParallelRefIterator, ParallelIterator},
    slice::ParallelSliceMut,
};
use serde::Deserialize;
use smol_str::SmolStr;
use std::fmt::Write;
use tap::Tap;

use crate::structs::CpuModel;

mod structs;

#[derive(Deserialize)]
struct InputData {
    data: Vec<serde_json::Value>,
}

fn main() -> anyhow::Result<()> {
    let raw_input: InputData = serde_json::from_str(include_str!("data.json"))?;
    eprintln!("loaded {} raw datapoints", raw_input.data.len());
    let mut input: Vec<CpuModel> = raw_input
        .data
        .par_iter()
        .filter_map(|v| CpuModel::from_json(v).ok())
        .collect();
    eprintln!("loaded {} clean datapoints", input.len());
    input.par_sort_unstable_by_key(|a| a.date);
    let absolute_fastest = input
        .iter()
        .reduce(|a, b| {
            if a.sequential_perf > b.sequential_perf {
                a
            } else {
                b
            }
        })
        .unwrap();

    // Gather all the unique dates
    let dates = input
        .iter()
        .skip(1)
        .map(|f| f.date)
        .collect::<Vec<_>>()
        .tap_mut(|v| v.dedup());
    // In parallel, calculate data
    let data: BTreeMap<NaiveDate, Datum> = dates
        .par_iter()
        .map(|date| {
            let frac = || input.iter().take_while(|c| &c.date <= date);
            let fastest = frac()
                .reduce(|a, b| {
                    if a.sequential_perf > b.sequential_perf {
                        a
                    } else {
                        b
                    }
                })
                .unwrap();
            let dosc_multiplier = fastest.sequential_perf / absolute_fastest.sequential_perf;
            let most_efficient = frac()
                .map(|cpu| {
                    let doscs_per_day = (cpu.sequential_perf / fastest.sequential_perf).powi(2);
                    let cost_per_day = cpu.daily_cost(*date);
                    (
                        doscs_per_day,
                        cost_per_day / doscs_per_day,
                        cpu.name.clone(),
                    )
                })
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                .unwrap();
            (
                *date,
                Datum {
                    max_speed: fastest.sequential_perf,
                    dosc_cost: most_efficient.1,
                    dosc_fraction: most_efficient.0,
                    dosc_multiplier,
                    model: most_efficient.2,
                    fastest_model: fastest.name.clone(),
                },
            )
        })
        .collect();
    // Create two CSVs

    // "Raw" CSV containing dosc stuff for every processor when it first comes out
    let mut raw_csv = String::new();
    writeln!(&mut raw_csv, "date,speed,dosc_cost,price")?;
    for cpu in input {
        if let Some(datum) = data.get(&cpu.date) {
            let doscs_per_day = (cpu.sequential_perf / datum.max_speed).powi(2);
            let cost_per_day = cpu.daily_cost(cpu.date);
            writeln!(
                &mut raw_csv,
                "{},{},{},{}",
                cpu.date,
                cpu.sequential_perf,
                (cost_per_day / doscs_per_day) * 100.0,
                cpu.raw_price
            )?;
        }
    }
    std::fs::write("raw.csv", raw_csv)?;

    // "Filtered CSV consisting of the best choices at every time"
    let mut filtered_csv = String::new();
    writeln!(
        &mut filtered_csv,
        "date,max_speed,dosc_cost,dosc_fraction,dosc_multiplier,model,fastest_model"
    )?;
    for (date, datum) in data {
        writeln!(
            &mut filtered_csv,
            "{},{},{},{},{},{},{}",
            date,
            datum.max_speed,
            datum.dosc_cost * 100.0,
            datum.dosc_fraction,
            datum.dosc_multiplier,
            datum.model,
            datum.fastest_model,
        )?;
    }
    std::fs::write("filtered.csv", filtered_csv)?;
    Ok(())
}

struct Datum {
    max_speed: f64,
    dosc_cost: f64,
    dosc_fraction: f64,
    dosc_multiplier: f64,
    model: SmolStr,
    fastest_model: SmolStr,
}
