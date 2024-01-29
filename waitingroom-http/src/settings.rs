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

#[settings(impl_default = false)]
pub(crate) struct WaitingRoomTimerSettings {
    /// The time in milliseconds between user count syncs across nodes.
    /// For the basic waiting room, this is a no-op.
    pub sync_user_counts_interval: u128,
    /// The time in milliseconds between cleanup operations.
    pub cleanup_interval: u128,
    /// The time in milliseconds between ensuring that correct number
    /// of users are on the site.
    pub ensure_correct_user_count_interval: u128,
}

impl Default for WaitingRoomTimerSettings {
    fn default() -> Self {
        Self {
            sync_user_counts_interval: 10 * 1000,
            cleanup_interval: 10 * 1000,
            ensure_correct_user_count_interval: 10 * 1000,
        }
    }
}

#[settings(impl_default = false)]
pub(crate) struct HttpServerSettings {
    /// Telemetry settings
    pub(crate) telemetry: TelemetrySettings,

    /// Basic waiting room settings
    pub(crate) waitingroom: BasicWaitingRoomSettings,

    /// Settings for the built-in demo HTTP server
    pub(crate) demo_http_server: DemoHTTPServerSettings,

    /// Timer settings
    pub(crate) timer: WaitingRoomTimerSettings,

    /// Cookie secret
    pub(crate) cookie_secret: String,

    /// Webserver listening address
    pub(crate) listening_address: SocketAddr,

    /// Address of the webserver behind the proxy
    pub(crate) proxy_address: SocketAddr,
}

impl Default for HttpServerSettings {
    fn default() -> Self {
        Self {
            telemetry: Default::default(),
            waitingroom: Default::default(),
            demo_http_server: Default::default(),
            timer: Default::default(),
            cookie_secret:
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            listening_address: SocketAddr::from(std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                8051,
            )),
            proxy_address: SocketAddr::from(std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                8052,
            )),
        }
    }
}
