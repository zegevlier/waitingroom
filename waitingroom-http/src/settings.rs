use foundations::settings::net::SocketAddr;
use foundations::settings::settings;
use foundations::telemetry::settings::TelemetrySettings;
use waitingroom_basic::BasicWaitingRoomSettings;

#[settings(impl_default = false)]
pub(crate) struct DemoHTTPServerSettings {
    /// Whether or not to enable the demo HTTP server
    pub(crate) enabled: bool,

    /// What address the demo HTTP server should be listening on.
    /// This is ignored if enabled is false.
    pub(crate) listening_address: SocketAddr,
}

impl Default for DemoHTTPServerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            listening_address: SocketAddr::from(std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                8052,
            )),
        }
    }
}

#[settings]
pub(crate) struct HttpServerSettings {
    /// Telemetry settings
    pub(crate) telemetry: TelemetrySettings,

    /// Basic waiting room settings
    pub(crate) waitingroom: BasicWaitingRoomSettings,

    /// Settings for the built-in demo HTTP server
    pub(crate) demo_http_server: DemoHTTPServerSettings,
}
