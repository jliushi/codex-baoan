//! 分词器指纹：把上游上报的 input_tokens 对各家族分词器计数做最小二乘，
//! 最贴合（slope≈1、残差最小、且与次佳有区分度）的家族即真实家族。

use crate::store::Sample;
use serde::Serialize;

const MIN_SAMPLES: usize = 8;
const SLOPE_LO: f64 = 0.85;
const SLOPE_HI: f64 = 1.20;
const SEPARATION: f64 = 1.3;

#[derive(Serialize, Clone)]
pub struct GroupVerdict {
    pub requested_model: String,
    pub reported_models: Vec<String>,
    pub samples: usize,
    pub family: String,       // o200k / cl100k / suspected_non_gpt / insufficient / ambiguous
    pub family_label: String,
    pub slope: f64,
    pub rmse: f64,
    pub note: String,
}

/// 普通最小二乘，返回 (slope, intercept, rmse)。
fn regress(xs: &[f64], ys: &[f64]) -> Option<(f64, f64, f64)> {
    let n = xs.len();
    if n < 2 || ys.len() != n {
        return None;
    }
    let nf = n as f64;
    let (mx, my) = (xs.iter().sum::<f64>() / nf, ys.iter().sum::<f64>() / nf);
    let sxx: f64 = xs.iter().map(|x| (x - mx) * (x - mx)).sum();
    if sxx <= 0.0 {
        return None;
    }
    let sxy: f64 = xs.iter().zip(ys).map(|(x, y)| (x - mx) * (y - my)).sum();
    let slope = sxy / sxx;
    let intercept = my - slope * mx;
    let rmse = (xs
        .iter()
        .zip(ys)
        .map(|(x, y)| {
            let d = y - (slope * x + intercept);
            d * d
        })
        .sum::<f64>()
        / nf)
        .sqrt();
    Some((slope, intercept, rmse))
}

/// 对一组同请求模型的样本给出家族判定。
pub fn fingerprint_group(reported: Vec<String>, o2: &[f64], cl: &[f64], ys: &[f64]) -> GroupVerdict {
    let mut v = GroupVerdict {
        requested_model: String::new(),
        reported_models: reported,
        samples: ys.len(),
        family: "insufficient".into(),
        family_label: "样本不足".into(),
        slope: 0.0,
        rmse: 0.0,
        note: String::new(),
    };
    if ys.len() < MIN_SAMPLES {
        v.note = format!("有效样本 {} < {}，拒绝下结论", ys.len(), MIN_SAMPLES);
        return v;
    }
    let (fo2, fcl) = match (regress(o2, ys), regress(cl, ys)) {
        (Some(a), Some(b)) => (a, b),
        _ => return v,
    };
    // 取残差更小的 GPT 分词器为最佳，另一个为对照。
    let (best, best_slope, best_rmse, best_label, other_rmse) = if fcl.2 < fo2.2 {
        ("cl100k", fcl.0, fcl.2, "老 GPT cl100k", fo2.2)
    } else {
        ("o200k", fo2.0, fo2.2, "GPT o200k (4o/5/6)", fcl.2)
    };
    v.slope = (best_slope * 1000.0).round() / 1000.0;
    v.rmse = (best_rmse * 10.0).round() / 10.0;
    let slope_ok = (SLOPE_LO..=SLOPE_HI).contains(&best_slope);
    let separated = other_rmse > best_rmse * SEPARATION;
    if slope_ok {
        v.family = best.into();
        v.family_label = best_label.into();
        v.note = if separated {
            format!("上报 input_tokens 最贴合 {best_label}（slope={best_slope:.3}）")
        } else {
            format!("贴合 {best_label}，但与另一 GPT 分词器区分度有限")
        };
    } else {
        v.family = "suspected_non_gpt".into();
        v.family_label = "疑似非 GPT 家族".into();
        v.note = format!(
            "上报 token 与 GPT 分词器都对不齐（最佳 slope={best_slope:.3}，偏离 1）→ 可能被换成非 GPT 模型，或请求体被中转改写"
        );
    }
    v
}

/// 按请求模型分组做指纹。
pub fn fingerprint_samples(samples: &[Sample]) -> Vec<GroupVerdict> {
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<String, (Vec<String>, Vec<f64>, Vec<f64>, Vec<f64>)> = BTreeMap::new();
    for s in samples {
        let g = groups.entry(s.requested_model.clone()).or_default();
        if !s.reported_model.is_empty() && !g.0.contains(&s.reported_model) {
            g.0.push(s.reported_model.clone());
        }
        if s.input_tokens > 0 && s.prompt_tokens_o200k > 0 {
            g.1.push(s.prompt_tokens_o200k as f64);
            g.2.push(s.prompt_tokens_cl100k as f64);
            g.3.push(s.input_tokens as f64);
        }
    }
    let mut out = Vec::new();
    for (model, (reported, o2, cl, ys)) in groups {
        let mut v = fingerprint_group(reported, &o2, &cl, &ys);
        v.requested_model = model;
        out.push(v);
    }
    out.sort_by(|a, b| b.samples.cmp(&a.samples));
    out
}
