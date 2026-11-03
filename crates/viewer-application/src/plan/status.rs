use std::collections::BTreeMap;

use viewer_domain::plan::RunRecord;

use super::compose::ComposedShot;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShotGeneration {
    /// 没有任何 run 记录覆盖这张分镜。
    Ungenerated,
    /// 最近一次 run 使用的哈希与当前拼装结果一致。
    Current,
    /// 最近一次 run 之后，计划里的这张分镜变了。
    Stale,
}

/// 用最近一次 run 的 `shotHash` 判定每张分镜的生成状态。纯函数，不读写磁盘。
pub fn shot_generation(
    shots: &[ComposedShot],
    runs: &[RunRecord],
) -> BTreeMap<String, ShotGeneration> {
    let mut ordered: Vec<&RunRecord> = runs.iter().collect();
    ordered.sort_by(|left, right| right.created_at.cmp(&left.created_at));

    // 每张分镜取"最新一条覆盖它的 run"；缺失的分镜回退到更早的 run。
    let mut latest: BTreeMap<&str, &str> = BTreeMap::new();
    for run in ordered {
        for shot in &run.shots {
            latest
                .entry(shot.shot_id.as_str())
                .or_insert(shot.shot_hash.as_str());
        }
    }

    shots
        .iter()
        .map(|shot| {
            let status = match latest.get(shot.shot_id.as_str()) {
                None => ShotGeneration::Ungenerated,
                Some(hash) if *hash == shot.hash => ShotGeneration::Current,
                Some(_) => ShotGeneration::Stale,
            };
            (shot.shot_id.clone(), status)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use viewer_domain::plan::{PlanId, Producer, Revision, RunShot};

    fn composed(shot_id: &str, hash: &str) -> ComposedShot {
        ComposedShot {
            shot_id: shot_id.to_owned(),
            prompt: String::new(),
            segments: Vec::new(),
            references: Vec::new(),
            hash: hash.to_owned(),
        }
    }

    fn run(created_at: &str, shots: &[(&str, &str)]) -> RunRecord {
        RunRecord {
            protocol_version: viewer_domain::plan::RUN_PROTOCOL_V1.to_owned(),
            run_id: format!("run-{created_at}"),
            plan_id: PlanId::parse("autumn").unwrap(),
            plan_revision: Revision::new(1),
            created_at: created_at.to_owned(),
            producer: Producer {
                name: "harness".to_owned(),
                session_id: None,
            },
            model: None,
            params: Default::default(),
            shots: shots
                .iter()
                .map(|(shot_id, hash)| RunShot {
                    shot_id: (*shot_id).to_owned(),
                    shot_hash: (*hash).to_owned(),
                    output: None,
                    reference_ids: Vec::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn a_shot_without_any_run_is_ungenerated() {
        let status = shot_generation(&[composed("s1", "h1")], &[]);
        assert_eq!(status["s1"], ShotGeneration::Ungenerated);
    }

    #[test]
    fn a_shot_whose_hash_matches_the_latest_run_is_current() {
        let status = shot_generation(&[composed("s1", "h1")], &[run("1", &[("s1", "h1")])]);
        assert_eq!(status["s1"], ShotGeneration::Current);
    }

    #[test]
    fn a_shot_whose_hash_changed_after_the_run_is_stale() {
        let status = shot_generation(&[composed("s1", "h2")], &[run("1", &[("s1", "h1")])]);
        assert_eq!(status["s1"], ShotGeneration::Stale);
    }

    #[test]
    fn the_latest_run_wins() {
        let status = shot_generation(
            &[composed("s1", "h2")],
            &[run("2", &[("s1", "h2")]), run("1", &[("s1", "h1")])],
        );
        assert_eq!(status["s1"], ShotGeneration::Current);
    }

    #[test]
    fn shots_missing_from_the_latest_run_fall_back_to_earlier_runs() {
        let status = shot_generation(
            &[composed("s1", "h1"), composed("s2", "h2")],
            &[run("2", &[("s1", "h1")]), run("1", &[("s2", "h2")])],
        );
        assert_eq!(status["s1"], ShotGeneration::Current);
        assert_eq!(status["s2"], ShotGeneration::Current);
    }
}
