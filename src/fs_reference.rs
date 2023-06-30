use crate::FsError;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Eq, PartialEq, PartialOrd, Ord, Debug, Clone)]
pub struct FsReference {
    pub project_id: String,
    pub database_id: String,
    pub path: FsPath,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, PartialOrd, Ord, Debug, Clone)]
pub struct FsPath(pub Vec<PathElement>);

#[derive(Serialize, Deserialize, Eq, PartialEq, PartialOrd, Ord, Debug, Clone)]
pub struct PathElement {
    collection_id: String,
    resource_id: Option<ResourceId>,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, PartialOrd, Ord, Debug, Clone)]
pub enum ResourceId {
    String(String),
    Number(i64),
}

impl FromStr for FsReference {
    type Err = FsError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let re =
            Regex::new(r"projects\/([-\w\d]+)\/databases\/([-\w\d]+)\/documents\/([-\w\/\d]*)")
                .unwrap();
        let cap = re
            .captures(s)
            .expect(&format!("Failed to parse {} as a fs reference", s));
        Ok(FsReference {
            project_id: cap[1].to_owned(),
            database_id: cap[2].to_owned(),
            path: FsPath::from_str(&cap[3])?,
        })
    }
}

impl fmt::Display for FsReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "projects/{}/databases/{}/{}",
            self.project_id, self.database_id, self.path
        )
    }
}

impl FsReference {
    fn is_root(&self) -> bool {
        self.path.0.is_empty()
    }
}

impl FromStr for ResourceId {
    type Err = FsError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(s.parse::<i64>()
            .map(|num| ResourceId::Number(num))
            .unwrap_or(ResourceId::String(s.to_string())))
    }
}

impl fmt::Display for FsPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.0
                .iter()
                .map(|path_element| path_element.to_string())
                .collect::<Vec<String>>()
                .join("/")
        )
    }
}

impl FromStr for FsPath {
    type Err = FsError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if s.is_empty() {
            return Ok(FsPath(vec![]));
        }
        let splits: Vec<&str> = s.split("/").collect();
        let mut paths: Vec<PathElement> = Vec::new();
        if splits.len() >= 2 {
            for i in (0..splits.len()).step_by(2) {
                paths.push(PathElement {
                    collection_id: splits[i].to_owned(),
                    resource_id: Some(ResourceId::from_str(splits[i + 1])?),
                })
            }
        }
        if splits.len() % 2 == 1 {
            paths.push(PathElement {
                collection_id: splits.last().unwrap().to_string(),
                resource_id: None,
            })
        }
        Ok(FsPath(paths))
    }
}

impl fmt::Display for PathElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // write!(f, "{}", self.0.iter().map(|path_element| path_element.to_string()).join("/"))
        match &self.resource_id {
            Some(resource_id) => write!(f, "{}/{}", self.collection_id, resource_id.to_string()),
            None => write!(f, "{}", self.collection_id),
        }
    }
}

impl fmt::Display for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceId::Number(num) => write!(f, "{}", num),
            ResourceId::String(string) => write!(f, "{}", string),
        }
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_fs_path() {
        assert_eq!(
            FsPath::from_str("users/1").unwrap(),
            FsPath(vec![PathElement {
                collection_id: "users".to_string(),
                resource_id: Some(ResourceId::Number(1))
            }])
        );
        assert_eq!(
            FsPath::from_str("users/abc").unwrap(),
            FsPath(vec![PathElement {
                collection_id: "users".to_string(),
                resource_id: Some(ResourceId::String("abc".to_string()))
            }])
        );
        assert_eq!(
            FsPath::from_str("users/1/Posts/1").unwrap(),
            FsPath(vec![
                PathElement {
                    collection_id: "users".to_string(),
                    resource_id: Some(ResourceId::Number(1))
                },
                PathElement {
                    collection_id: "Posts".to_string(),
                    resource_id: Some(ResourceId::Number(1))
                }
            ])
        );
        assert_eq!(
            FsPath::from_str("users").unwrap(),
            FsPath(vec![PathElement {
                collection_id: "users".to_string(),
                resource_id: None
            }])
        );
    }

    #[test]
    fn test_fs_reference() {
        assert_eq!(
            FsReference::from_str(
                "projects/test-project/databases/test-database/documents/users/1"
            )
            .unwrap(),
            FsReference {
                project_id: "test-project".to_string(),
                database_id: "test-database".to_string(),
                path: FsPath(vec![PathElement {
                    collection_id: "users".to_string(),
                    resource_id: Some(ResourceId::Number(1))
                }])
            }
        );
        assert_eq!(
            FsReference::from_str("projects/test-project/databases/test-database/documents/users")
                .unwrap(),
            FsReference {
                project_id: "test-project".to_string(),
                database_id: "test-database".to_string(),
                path: FsPath(vec![PathElement {
                    collection_id: "users".to_string(),
                    resource_id: None,
                }])
            }
        );
        assert_eq!(
            FsReference::from_str("projects/test-project/databases/test-database/documents/")
                .unwrap(),
            FsReference {
                project_id: "test-project".to_string(),
                database_id: "test-database".to_string(),
                path: FsPath(vec![])
            }
        );
        assert!(
            FsReference::from_str("projects/test-project/databases/test-database/documents/")
                .unwrap()
                .is_root()
        )
    }
}
