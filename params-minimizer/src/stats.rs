use std::time;

#[derive(Debug, Clone)]
pub struct BenchValues {
    pub mean: f32,
    pub deviation: f32,
}

pub fn mean(data: &Vec<f32>) -> Option<f32> {
    let sum = data.iter().sum::<f32>() as f32;
    let count = data.len();

    match count {
        positive if positive > 0 => Some(sum / count as f32),
        _ => None,
    }
}

pub fn std_deviation(data: &Vec<f32>) -> Option<f32> {
    match (mean(data), data.len()) {
        (Some(data_mean), count) if count > 0 => {
            let variance = data
                .iter()
                .map(|value| {
                    let diff = data_mean - (*value as f32);

                    diff * diff
                })
                .sum::<f32>()
                / count as f32;

            Some(variance.sqrt())
        }
        _ => None,
    }
}

pub fn compute_stats(all_durations: Vec<Vec<time::Duration>>) -> BenchValues {
    log::debug!("All durations: {:?}", all_durations);
    let mean_cert_per_node = all_durations
        .iter()
        .map(|node_times| mean(&node_times.iter().map(|t| t.as_millis() as f32).collect()).unwrap())
        .collect::<Vec<_>>();

    let mean_across_nodes = mean(&mean_cert_per_node);
    let deviation_across_nodes = std_deviation(&mean_cert_per_node);

    BenchValues {
        mean: mean_across_nodes.unwrap(),
        deviation: deviation_across_nodes.unwrap(),
    }
}
