//! A small persistent record of changes, with links back to the affected data.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Activity {
    pub id: u64,
    pub at: u64,
    pub message: String,
    pub target: Option<ActivityTarget>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActivityTarget {
    Person {
        id: usize,
    },
    People {
        ids: Vec<usize>,
    },
    Category {
        category_kind: String,
        spelling: String,
    },
    Tables {
        ids: Vec<String>,
    },
    Option {
        index: usize,
    },
}

impl Project {
    pub fn record_activity(&mut self, at: u64, message: String, target: Option<ActivityTarget>) {
        let id = self.activity.last().map_or(1, |a| a.id + 1);
        self.activity.push(Activity {
            id,
            at,
            message,
            target,
        });
        if self.activity.len() > 200 {
            self.activity.drain(..self.activity.len() - 200);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activity_stays_bounded_and_persists_without_reusing_ids() {
        let mut project = Project::new();
        for i in 0..240 {
            project.record_activity(i, "X".into(), None);
        }
        assert_eq!(project.activity.len(), 200);
        assert_eq!(project.activity.first().unwrap().id, 41);
        project.validate().unwrap();
        let decoded: Project =
            serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
        assert_eq!(decoded.activity.last().unwrap().id, 240);
    }
}
