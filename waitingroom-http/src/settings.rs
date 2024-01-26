use foundations::settings::settings;
use foundations::telemetry::settings::TelemetrySettings;
use waitingroom_basic::BasicWaitingRoomSettings;

#[settings]
pub(crate) struct HttpServerSettings {
    /// Telemetry settings
    pub(crate) telemetry: TelemetrySettings,

    /// Basic waiting room settings
    pub(crate) waiting_room: BasicWaitingRoomSettings,
}
