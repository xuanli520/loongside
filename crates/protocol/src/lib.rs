use std::collections::BTreeMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportInfo {
    pub name: String,
    pub version: String,
    pub secure: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InboundFrame {
    pub method: String,
    pub id: Option<String>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutboundFrame {
    pub method: String,
    pub id: Option<String>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolRoute {
    Initialize,
    Ping,
    ToolsList,
    ToolsCall,
    ResourcesList,
    ResourcesRead,
    Custom(String),
}

impl ProtocolRoute {
    pub fn from_method(method: &str) -> Self {
        match method {
            "initialize" => Self::Initialize,
            "ping" => Self::Ping,
            "tools/list" => Self::ToolsList,
            "tools/call" => Self::ToolsCall,
            "resources/list" => Self::ResourcesList,
            "resources/read" => Self::ResourcesRead,
            other => Self::Custom(other.to_owned()),
        }
    }

    pub fn method(&self) -> &str {
        match self {
            Self::Initialize => "initialize",
            Self::Ping => "ping",
            Self::ToolsList => "tools/list",
            Self::ToolsCall => "tools/call",
            Self::ResourcesList => "resources/list",
            Self::ResourcesRead => "resources/read",
            Self::Custom(method) => method,
        }
    }

    pub fn is_standard(&self) -> bool {
        !matches!(self, Self::Custom(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutePolicy {
    pub allow_anonymous: bool,
    pub required_capability: Option<String>,
}

impl Default for RoutePolicy {
    fn default() -> Self {
        Self {
            allow_anonymous: true,
            required_capability: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRoute {
    pub route: ProtocolRoute,
    pub policy: RoutePolicy,
}

#[derive(Debug, Clone)]
pub struct ProtocolRouter {
    strict: bool,
    custom_routes: BTreeMap<String, RoutePolicy>,
}

impl Default for ProtocolRouter {
    fn default() -> Self {
        Self::new(false)
    }
}

impl ProtocolRouter {
    pub fn new(strict: bool) -> Self {
        Self {
            strict,
            custom_routes: BTreeMap::new(),
        }
    }

    pub fn strict() -> Self {
        Self::new(true)
    }

    pub fn register_custom_route(
        &mut self,
        method: impl Into<String>,
        policy: RoutePolicy,
    ) -> Result<(), RouterError> {
        let method = method.into();
        if method.trim().is_empty() {
            return Err(RouterError::InvalidMethod(
                "custom route method cannot be empty".to_owned(),
            ));
        }
        if ProtocolRoute::from_method(&method).is_standard() {
            return Err(RouterError::InvalidMethod(format!(
                "standard route cannot be registered as custom: {method}"
            )));
        }
        self.custom_routes.insert(method, policy);
        Ok(())
    }

    pub fn resolve(&self, method: &str) -> Result<ResolvedRoute, RouterError> {
        let route = ProtocolRoute::from_method(method);
        match route {
            ProtocolRoute::Initialize | ProtocolRoute::Ping => Ok(ResolvedRoute {
                route,
                policy: RoutePolicy {
                    allow_anonymous: true,
                    required_capability: None,
                },
            }),
            ProtocolRoute::ToolsList | ProtocolRoute::ResourcesList => Ok(ResolvedRoute {
                route,
                policy: RoutePolicy {
                    allow_anonymous: true,
                    required_capability: Some("discover".to_owned()),
                },
            }),
            ProtocolRoute::ToolsCall | ProtocolRoute::ResourcesRead => Ok(ResolvedRoute {
                route,
                policy: RoutePolicy {
                    allow_anonymous: false,
                    required_capability: Some("invoke".to_owned()),
                },
            }),
            ProtocolRoute::Custom(custom) => {
                if let Some(policy) = self.custom_routes.get(&custom) {
                    Ok(ResolvedRoute {
                        route: ProtocolRoute::Custom(custom),
                        policy: policy.clone(),
                    })
                } else if self.strict {
                    Err(RouterError::UnknownMethod(custom))
                } else {
                    Ok(ResolvedRoute {
                        route: ProtocolRoute::Custom(custom),
                        policy: RoutePolicy::default(),
                    })
                }
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum RouterError {
    #[error("unknown protocol method: {0}")]
    UnknownMethod(String),
    #[error("invalid protocol method: {0}")]
    InvalidMethod(String),
}

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("transport closed")]
    Closed,
    #[error("transport failure: {0}")]
    Failure(String),
}

#[async_trait]
pub trait Transport: Send + Sync + 'static {
    fn info(&self) -> TransportInfo;
    async fn send(&self, frame: OutboundFrame) -> Result<(), TransportError>;
    async fn recv(&self) -> Result<Option<InboundFrame>, TransportError>;
    async fn close(&self) -> Result<(), TransportError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_parser_covers_standard_methods() {
        assert_eq!(
            ProtocolRoute::from_method("tools/call"),
            ProtocolRoute::ToolsCall
        );
        assert_eq!(
            ProtocolRoute::from_method("resources/read"),
            ProtocolRoute::ResourcesRead
        );
        assert_eq!(
            ProtocolRoute::from_method("custom/x"),
            ProtocolRoute::Custom("custom/x".to_owned())
        );
    }

    #[test]
    fn strict_router_rejects_unknown_custom_methods() {
        let router = ProtocolRouter::strict();
        let error = router
            .resolve("internal/unsafe")
            .expect_err("strict mode should block unknown methods");
        assert!(matches!(error, RouterError::UnknownMethod(method) if method == "internal/unsafe"));
    }

    #[test]
    fn custom_route_policy_is_applied() {
        let mut router = ProtocolRouter::strict();
        router
            .register_custom_route(
                "channel/publish",
                RoutePolicy {
                    allow_anonymous: false,
                    required_capability: Some("channel.publish".to_owned()),
                },
            )
            .expect("custom route registration should succeed");

        let resolved = router
            .resolve("channel/publish")
            .expect("registered custom route should resolve");
        assert_eq!(
            resolved.route,
            ProtocolRoute::Custom("channel/publish".to_owned())
        );
        assert!(!resolved.policy.allow_anonymous);
        assert_eq!(
            resolved.policy.required_capability.as_deref(),
            Some("channel.publish")
        );
    }
}
