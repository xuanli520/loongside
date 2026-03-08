#[derive(Debug, Clone)]
pub(in crate::mvp::channel::feishu) struct FeishuInboundEvent {
    pub(in crate::mvp::channel::feishu) event_id: String,
    pub(in crate::mvp::channel::feishu) session_id: String,
    pub(in crate::mvp::channel::feishu) message_id: String,
    pub(in crate::mvp::channel::feishu) text: String,
}

#[derive(Debug)]
pub(in crate::mvp::channel::feishu) enum FeishuWebhookAction {
    UrlVerification { challenge: String },
    Ignore,
    Inbound(FeishuInboundEvent),
}
