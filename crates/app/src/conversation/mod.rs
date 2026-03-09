mod orchestrator;
mod persistence;
mod runtime;
pub mod turn_engine;

pub use orchestrator::ConversationOrchestrator;
#[allow(unused_imports)]
pub use runtime::{ConversationRuntime, DefaultConversationRuntime};
pub use turn_engine::{
    ProviderTurn, ToolDecision, ToolIntent, ToolOutcome, TurnEngine, TurnResult,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderErrorMode {
    #[cfg_attr(
        not(any(feature = "channel-telegram", feature = "channel-feishu")),
        allow(dead_code)
    )]
    Propagate,
    InlineMessage,
}

#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod tests;
