use serde_json::{json, Value};

pub fn percentile(samples: &[u64], percent: usize) -> u64 {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    sorted[((sorted.len() - 1) * percent).div_ceil(100)]
}

/// Aggregate measured windows, excluding cohort setup, discovery, warmup and teardown.
pub fn aggregate(reports: Vec<Value>, iterations: usize, warmup: usize) -> Value {
    if reports.len() == 1 {
        return reports.into_iter().next().unwrap();
    }
    let count = reports.len();
    let mut result = reports[0].clone();
    let total = (iterations + warmup) * count;
    result["iterations"] = json!(iterations * count);
    result["warmup"] = json!(warmup * count);
    for field in ["received", "accepted", "feedback", "results"] {
        result[field] = json!(total);
    }
    result["canceled"] = json!(if result["scenario"] == 1 { total } else { 0 });
    result["checksum"] = json!(total as u64 * (total as u64 - 1) / 2);
    let mut cohorts = Vec::with_capacity(count);
    for (index, report) in reports.iter().enumerate() {
        let mut cohort = json!({
            "index":index,"sequence_offset":index*(iterations+warmup),
            "iterations":iterations,"warmup":warmup,
            "received":report["received"],"accepted":report["accepted"],
            "feedback":report["feedback"],"results":report["results"],
            "canceled":report["canceled"],"checksum":report["checksum"]
        });
        if report["role"] == "ping" {
            cohort["elapsed_ns"] = report["elapsed_ns"].clone();
        }
        if let Some(probe) = report.get("probe") {
            cohort["probe"] = probe.clone();
        }
        cohorts.push(cohort);
    }
    result["cohort_config"] = json!({"count":count,"iterations":iterations,"warmup":warmup,
        "maximum_retained_goals":iterations+warmup,"fresh_endpoints":true});
    result["cohorts"] = json!(cohorts);
    if result["role"] == "ping" {
        let concatenate = |field: &str, stage: bool| -> Vec<u64> {
            reports
                .iter()
                .flat_map(|report| {
                    let values = if stage {
                        &report["stage_samples_ns"][field]
                    } else {
                        &report[field]
                    };
                    values
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| value.as_u64().unwrap())
                })
                .collect()
        };
        let samples = concatenate("samples_ns", false);
        let acceptance = concatenate("acceptance", true);
        let cancellation = concatenate("cancellation", true);
        let elapsed: u64 = reports
            .iter()
            .map(|report| report["elapsed_ns"].as_u64().unwrap())
            .sum();
        result["elapsed_ns"] = json!(elapsed);
        result["roundtrips_per_second"] =
            json!(iterations as f64 * count as f64 / (elapsed as f64 * 1e-9));
        for percent in [50, 95, 99] {
            result[format!("rtt_ns_p{percent}")] = json!(percentile(&samples, percent));
        }
        result["rtt_ns_max"] = json!(samples.iter().max().unwrap());
        result["acceptance_ns_p50"] = json!(percentile(&acceptance, 50));
        result["cancellation_ns_p50"] = json!(if cancellation.is_empty() {
            None
        } else {
            Some(percentile(&cancellation, 50))
        });
        result["samples_ns"] = json!(samples);
        result["stage_samples_ns"] = json!({"acceptance":acceptance,"cancellation":cancellation});
    }
    // Per-cohort probe frames must remain separate; they have independently reset counters.
    result.as_object_mut().unwrap().remove("probe");
    result
}
