use crate::FsError;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Eq, PartialEq, PartialOrd, Ord, Debug, Clone)]
pub struct FsReference {
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
        let re = Regex::new(r"\/([-\w\/\d]*)").unwrap();
        let cap = re
            .captures(s)
            .expect(&format!("Failed to parse {} as a fs reference", s));
        Ok(FsReference {
            path: FsPath::from_str(&cap[1])?,
        })
    }
}

impl fmt::Display for FsReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/{}", self.path)
    }
}

impl FsReference {
    pub fn is_root(&self) -> bool {
        self.path.0.is_empty()
    }

    pub fn has_complete_path(&self) -> bool {
        if self.path.0.is_empty() {
            true
        } else {
            self.path.0.last().unwrap().resource_id.is_some()
        }
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
            FsReference::from_str("/users/1").unwrap(),
            FsReference {
                path: FsPath(vec![PathElement {
                    collection_id: "users".to_string(),
                    resource_id: Some(ResourceId::Number(1))
                }])
            }
        );
        assert_eq!(
            FsReference::from_str("/users").unwrap(),
            FsReference {
                path: FsPath(vec![PathElement {
                    collection_id: "users".to_string(),
                    resource_id: None,
                }])
            }
        );
        assert_eq!(
            FsReference::from_str("/").unwrap(),
            FsReference {
                path: FsPath(vec![])
            }
        );
        assert!(FsReference::from_str("/").unwrap().is_root());
        assert_eq!(
            FsReference::from_str("/users").unwrap().has_complete_path(),
            false
        );
        assert_eq!(
            FsReference::from_str("/users/1")
                .unwrap()
                .has_complete_path(),
            true
        )
    }
}
