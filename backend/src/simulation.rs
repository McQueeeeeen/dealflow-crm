use rand::Rng;
use serde::Serialize;

const PRIOR_WEIGHT: f64 = 5.0;
const FALLBACK_DWELL_DAYS: f64 = 14.0;

#[derive(Debug, Clone)]
pub struct StageModel {
    pub advance: f64,
    dwell_days: Vec<f64>,
}

#[derive(Debug, Clone, Copy)]
pub struct OpenDeal {
    pub value_cents: i64,
    pub stage: usize,
    pub days_in_stage: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct Transition {
    pub days: f64,
    pub advanced: bool,
}

#[derive(Debug, Serialize)]
pub struct Bin {
    pub from_cents: i64,
    pub to_cents: i64,
    pub runs: usize,
}

#[derive(Debug, Serialize)]
pub struct Distribution {
    pub mean_cents: i64,
    pub p10_cents: i64,
    pub p50_cents: i64,
    pub p90_cents: i64,
    pub histogram: Vec<Bin>,
}

impl StageModel {
    pub fn learn(transitions: &[Transition], prior_advance: f64) -> Self {
        let advanced = transitions.iter().filter(|t| t.advanced).count() as f64;
        let advance = (advanced + PRIOR_WEIGHT * prior_advance.clamp(0.02, 0.98))
            / (transitions.len() as f64 + PRIOR_WEIGHT);
        let mut dwell_days: Vec<f64> = transitions.iter().map(|t| t.days.max(0.0)).collect();
        if dwell_days.is_empty() {
            dwell_days.push(FALLBACK_DWELL_DAYS);
        }
        dwell_days.sort_by(f64::total_cmp);
        Self {
            advance,
            dwell_days,
        }
    }

    pub fn median_days(&self) -> f64 {
        self.dwell_days[self.dwell_days.len() / 2]
    }

    fn remaining_days<R: Rng>(&self, rng: &mut R, elapsed: f64) -> f64 {
        let longer = self.dwell_days.partition_point(|d| *d <= elapsed);
        if longer < self.dwell_days.len() {
            self.dwell_days[rng.gen_range(longer..self.dwell_days.len())] - elapsed
        } else {
            self.dwell_days[rng.gen_range(0..self.dwell_days.len())]
        }
    }
}

fn close_day<R: Rng>(stages: &[StageModel], deal: &OpenDeal, limit: f64, rng: &mut R) -> Option<f64> {
    let mut stage = deal.stage;
    let mut day = stages.get(stage)?.remaining_days(rng, deal.days_in_stage);
    loop {
        if day > limit || !rng.gen_bool(stages[stage].advance) {
            return None;
        }
        stage += 1;
        if stage == stages.len() {
            return Some(day);
        }
        day += stages[stage].remaining_days(rng, 0.0);
    }
}

pub fn simulate<R: Rng>(
    stages: &[StageModel],
    deals: &[OpenDeal],
    horizons: &[f64],
    runs: usize,
    rng: &mut R,
) -> Vec<Vec<i64>> {
    let limit = horizons.iter().copied().fold(0.0, f64::max);
    let mut totals = vec![vec![0_i64; runs]; horizons.len()];
    for run in 0..runs {
        for deal in deals {
            if let Some(day) = close_day(stages, deal, limit, rng) {
                for (h, horizon) in horizons.iter().enumerate() {
                    if day <= *horizon {
                        totals[h][run] += deal.value_cents;
                    }
                }
            }
        }
    }
    totals
}

pub fn summarize(totals: &mut [i64], bins: usize) -> Distribution {
    totals.sort_unstable();
    let at = |p: f64| totals[((totals.len() - 1) as f64 * p).round() as usize];
    let (low, high) = (totals[0], at(0.99).max(totals[0] + 1));
    let width = ((high - low) as f64 / bins as f64).ceil().max(1.0) as i64;
    let mut histogram: Vec<Bin> = (0..bins as i64)
        .map(|i| Bin {
            from_cents: low + i * width,
            to_cents: low + (i + 1) * width,
            runs: 0,
        })
        .collect();
    for value in totals.iter() {
        let index = (((value - low) / width) as usize).min(bins - 1);
        histogram[index].runs += 1;
    }
    Distribution {
        mean_cents: totals.iter().sum::<i64>() / totals.len() as i64,
        p10_cents: at(0.1),
        p50_cents: at(0.5),
        p90_cents: at(0.9),
        histogram,
    }
}

pub fn share_at_least(sorted: &[i64], target: i64) -> f64 {
    let below = sorted.partition_point(|v| *v < target);
    (sorted.len() - below) as f64 / sorted.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn certain(days: f64) -> StageModel {
        StageModel {
            advance: 1.0,
            dwell_days: vec![days],
        }
    }

    #[test]
    fn learning_shrinks_thin_evidence_towards_the_prior() {
        let one_win = [Transition {
            days: 3.0,
            advanced: true,
        }];
        let model = StageModel::learn(&one_win, 0.5);
        assert!((model.advance - (1.0 + 2.5) / 6.0).abs() < 1e-9);

        let many: Vec<Transition> = (0..1000)
            .map(|i| Transition {
                days: f64::from(i),
                advanced: i % 4 == 0,
            })
            .collect();
        let model = StageModel::learn(&many, 0.9);
        assert!((model.advance - 0.25).abs() < 0.01);
        assert_eq!(model.median_days(), 500.0);
    }

    #[test]
    fn a_certain_path_closes_on_the_sum_of_dwell_times() {
        let stages = [certain(5.0), certain(10.0)];
        let deal = OpenDeal {
            value_cents: 700,
            stage: 0,
            days_in_stage: 2.0,
        };
        let mut rng = StdRng::seed_from_u64(1);
        assert_eq!(close_day(&stages, &deal, 365.0, &mut rng), Some(13.0));
        assert_eq!(close_day(&stages, &deal, 12.0, &mut rng), None);

        let totals = simulate(&stages, &[deal], &[12.0, 13.0], 4, &mut rng);
        assert_eq!(totals, vec![vec![0; 4], vec![700; 4]]);
    }

    #[test]
    fn hopeless_stages_never_close() {
        let stages = [StageModel {
            advance: 0.0,
            dwell_days: vec![1.0],
        }];
        let deal = OpenDeal {
            value_cents: 1,
            stage: 0,
            days_in_stage: 0.0,
        };
        let mut rng = StdRng::seed_from_u64(2);
        let totals = simulate(&stages, &[deal], &[90.0], 100, &mut rng);
        assert!(totals[0].iter().all(|v| *v == 0));
    }

    #[test]
    fn coin_flip_deals_land_near_half_their_value() {
        let stages = [StageModel {
            advance: 0.5,
            dwell_days: vec![1.0],
        }];
        let deals = vec![
            OpenDeal {
                value_cents: 100,
                stage: 0,
                days_in_stage: 0.0,
            };
            50
        ];
        let mut rng = StdRng::seed_from_u64(3);
        let mut totals = simulate(&stages, &deals, &[30.0], 4000, &mut rng).remove(0);
        let summary = summarize(&mut totals, 10);
        assert!((summary.mean_cents - 2500).abs() < 60, "{summary:?}");
        assert!(summary.p10_cents < summary.p50_cents && summary.p50_cents < summary.p90_cents);
        assert_eq!(summary.histogram.iter().map(|b| b.runs).sum::<usize>(), 4000);
        assert!((share_at_least(&totals, 2500) - 0.55).abs() < 0.05);
    }

    #[test]
    fn summary_of_identical_runs_is_flat() {
        let mut totals = vec![42; 10];
        let summary = summarize(&mut totals, 4);
        assert_eq!((summary.p10_cents, summary.p90_cents), (42, 42));
        assert_eq!(summary.histogram[0].runs, 10);
        assert_eq!(share_at_least(&totals, 43), 0.0);
    }
}
