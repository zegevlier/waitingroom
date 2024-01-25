use foundations::settings::settings;
use foundations::telemetry::settings::TelemetrySettings;

#[settings]
pub(crate) struct HttpServerSettings {
    /// Telemetry settings
    pub(crate) telemetry: TelemetrySettings,
}
