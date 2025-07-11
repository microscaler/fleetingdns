//! Graceful shutdown framework for FleetingDNS daemon binaries.
//!
//! Provides unified signal handling, Unix socket control interface, and
//! resource cleanup coordination across all FleetingDNS services.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use tokio::net::{UnixListener, UnixStream};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tracing::{error, info, warn};

use crate::{AppError, AppResult};

/// Shutdown signal types with different urgency levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShutdownSignal {
    /// Graceful shutdown - allow in-flight requests to complete (30s timeout)
    Graceful,
    /// Immediate shutdown - stop accepting new requests, finish current (5s timeout)
    Immediate,
    /// Force shutdown - terminate immediately (1s timeout)
    Force,
}

/// Current state of the shutdown process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShutdownState {
    /// Service is running normally
    Running,
    /// Service is draining connections, no new requests accepted
    Draining,
    /// Service is cleaning up resources
    Stopping,
    /// Service has stopped
    Stopped,
}

/// Control commands sent via Unix socket.
#[derive(Debug, Serialize, Deserialize)]
pub enum ControlCommand {
    /// Shutdown with specified signal type
    Shutdown { signal: ShutdownSignal },
    /// Get current status
    Status,
    /// Ping for health check
    Ping,
    /// Reload configuration (future use)
    Reload,
}

/// Response to control commands.
#[derive(Debug, Serialize, Deserialize)]
pub struct ControlResponse {
    /// Human-readable status message
    pub status: String,
    /// Service uptime
    pub uptime: Duration,
    /// Number of active connections
    pub active_connections: u64,
    /// Current shutdown state
    pub shutdown_state: ShutdownState,
    /// Component name
    pub component: String,
}

/// Configuration for graceful shutdown behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownConfig {
    /// Path to Unix control socket
    pub control_socket_path: PathBuf,
    /// Timeout for graceful shutdown
    pub graceful_timeout: Duration,
    /// Timeout for immediate shutdown
    pub immediate_timeout: Duration,
    /// Timeout for force shutdown
    pub force_timeout: Duration,
    /// Component name for logging and identification
    pub component_name: String,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            control_socket_path: get_default_socket_path("unknown"),
            graceful_timeout: Duration::from_secs(30),
            immediate_timeout: Duration::from_secs(5),
            force_timeout: Duration::from_secs(1),
            component_name: "unknown".to_string(),
        }
    }
}

/// Main graceful shutdown coordinator.
pub struct GracefulShutdown {
    pub config: ShutdownConfig,
    state: Arc<Mutex<ShutdownState>>,
    shutdown_tx: broadcast::Sender<ShutdownSignal>,
    active_connections: Arc<AtomicU64>,
    start_time: SystemTime,
    control_handle: Option<JoinHandle<()>>,
}

impl GracefulShutdown {
    /// Create a new graceful shutdown coordinator.
    pub fn new(component_name: &str) -> AppResult<Self> {
        let config = ShutdownConfig {
            control_socket_path: get_default_socket_path(component_name),
            component_name: component_name.to_string(),
            ..Default::default()
        };

        Self::with_config(config)
    }

    /// Create with custom configuration.
    pub fn with_config(config: ShutdownConfig) -> AppResult<Self> {
        let (shutdown_tx, _) = broadcast::channel(16);

        Ok(Self {
            config,
            state: Arc::new(Mutex::new(ShutdownState::Running)),
            shutdown_tx,
            active_connections: Arc::new(AtomicU64::new(0)),
            start_time: SystemTime::now(),
            control_handle: None,
        })
    }

    /// Start the shutdown framework (signal handlers and control socket).
    pub async fn start(&mut self) -> AppResult<()> {
        info!(
            component = %self.config.component_name,
            socket_path = %self.config.control_socket_path.display(),
            "Starting graceful shutdown framework"
        );

        // Start signal handlers
        self.start_signal_handlers().await?;

        // Start Unix socket control interface
        self.start_control_socket().await?;

        Ok(())
    }

    /// Subscribe to shutdown signals.
    pub fn subscribe(&self) -> broadcast::Receiver<ShutdownSignal> {
        self.shutdown_tx.subscribe()
    }

    /// Get current shutdown state.
    pub fn state(&self) -> ShutdownState {
        *self.state.lock().unwrap()
    }

    /// Increment active connection count.
    pub fn connection_started(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement active connection count.
    pub fn connection_finished(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get current active connection count.
    pub fn active_connections(&self) -> u64 {
        self.active_connections.load(Ordering::Relaxed)
    }

    /// Trigger shutdown with specified signal.
    pub async fn shutdown(&self, signal: ShutdownSignal) -> AppResult<()> {
        info!(
            component = %self.config.component_name,
            signal = ?signal,
            "Initiating shutdown"
        );

        // Update state
        {
            let mut state = self.state.lock().unwrap();
            *state = match signal {
                ShutdownSignal::Force => ShutdownState::Stopping,
                _ => ShutdownState::Draining,
            };
        }

        // Broadcast shutdown signal
        if let Err(e) = self.shutdown_tx.send(signal) {
            warn!("Failed to broadcast shutdown signal: {}", e);
        }

        Ok(())
    }

    /// Wait for shutdown to complete with timeout.
    pub async fn wait_for_shutdown(&self) -> AppResult<()> {
        let timeout_duration = match self.state() {
            ShutdownState::Draining => self.config.graceful_timeout,
            ShutdownState::Stopping => self.config.immediate_timeout,
            _ => self.config.force_timeout,
        };

        info!(
            component = %self.config.component_name,
            timeout = ?timeout_duration,
            "Waiting for shutdown to complete"
        );

        // Wait for connections to drain
        let result = timeout(timeout_duration, async {
            while self.active_connections() > 0 {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;

        if result.is_err() {
            warn!(
                component = %self.config.component_name,
                active_connections = self.active_connections(),
                "Shutdown timeout reached, forcing termination"
            );
        }

        // Update final state
        {
            let mut state = self.state.lock().unwrap();
            *state = ShutdownState::Stopped;
        }

        info!(
            component = %self.config.component_name,
            "Shutdown complete"
        );

        Ok(())
    }

    async fn start_signal_handlers(&self) -> AppResult<()> {
        let shutdown_tx = self.shutdown_tx.clone();
        let component_name = self.config.component_name.clone();

        // SIGTERM - Graceful shutdown
        let mut sigterm = signal(SignalKind::terminate())?;
        let shutdown_tx_term = shutdown_tx.clone();
        let component_term = component_name.clone();
        tokio::spawn(async move {
            sigterm.recv().await;
            info!(component = %component_term, "Received SIGTERM, initiating graceful shutdown");
            let _ = shutdown_tx_term.send(ShutdownSignal::Graceful);
        });

        // SIGINT - Graceful shutdown (Ctrl+C)
        let mut sigint = signal(SignalKind::interrupt())?;
        let shutdown_tx_int = shutdown_tx.clone();
        let component_int = component_name.clone();
        tokio::spawn(async move {
            sigint.recv().await;
            info!(component = %component_int, "Received SIGINT, initiating graceful shutdown");
            let _ = shutdown_tx_int.send(ShutdownSignal::Graceful);
        });

        // SIGUSR1 - Immediate shutdown
        let mut sigusr1 = signal(SignalKind::user_defined1())?;
        let shutdown_tx_usr1 = shutdown_tx.clone();
        let component_usr1 = component_name.clone();
        tokio::spawn(async move {
            sigusr1.recv().await;
            info!(component = %component_usr1, "Received SIGUSR1, initiating immediate shutdown");
            let _ = shutdown_tx_usr1.send(ShutdownSignal::Immediate);
        });

        // SIGUSR2 - Force shutdown
        let mut sigusr2 = signal(SignalKind::user_defined2())?;
        let shutdown_tx_usr2 = shutdown_tx;
        let component_usr2 = component_name;
        tokio::spawn(async move {
            sigusr2.recv().await;
            info!(component = %component_usr2, "Received SIGUSR2, initiating force shutdown");
            let _ = shutdown_tx_usr2.send(ShutdownSignal::Force);
        });

        Ok(())
    }

    async fn start_control_socket(&mut self) -> AppResult<()> {
        // Ensure socket directory exists
        if let Some(parent) = self.config.control_socket_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Remove existing socket file if it exists
        let _ = tokio::fs::remove_file(&self.config.control_socket_path).await;

        let listener = UnixListener::bind(&self.config.control_socket_path)?;

        let state = self.state.clone();
        let active_connections = self.active_connections.clone();
        let start_time = self.start_time;
        let component_name = self.config.component_name.clone();
        let shutdown_tx = self.shutdown_tx.clone();

        let handle = tokio::spawn(async move {
            info!(
                component = %component_name,
                "Control socket listening for commands"
            );

            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let state = state.clone();
                        let active_connections = active_connections.clone();
                        let component_name = component_name.clone();
                        let shutdown_tx = shutdown_tx.clone();

                        tokio::spawn(async move {
                            if let Err(e) = handle_control_connection(
                                stream,
                                state,
                                active_connections,
                                start_time,
                                component_name,
                                shutdown_tx,
                            )
                            .await
                            {
                                error!("Control connection error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept control connection: {}", e);
                        break;
                    }
                }
            }
        });

        self.control_handle = Some(handle);
        Ok(())
    }
}

impl Drop for GracefulShutdown {
    fn drop(&mut self) {
        // Clean up control socket
        let _ = std::fs::remove_file(&self.config.control_socket_path);

        // Abort control handle
        if let Some(handle) = &self.control_handle {
            handle.abort();
        }
    }
}

async fn handle_control_connection(
    stream: UnixStream,
    state: Arc<Mutex<ShutdownState>>,
    active_connections: Arc<AtomicU64>,
    start_time: SystemTime,
    component_name: String,
    shutdown_tx: broadcast::Sender<ShutdownSignal>,
) -> AppResult<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    if reader.read_line(&mut line).await? == 0 {
        return Ok(()); // Connection closed
    }

    let command: ControlCommand = serde_json::from_str(line.trim())
        .map_err(|e| AppError::Message(format!("Invalid command: {e}")))?;

    let response = match command {
        ControlCommand::Shutdown { signal } => {
            info!(
                component = %component_name,
                signal = ?signal,
                "Control socket shutdown command received"
            );

            let _ = shutdown_tx.send(signal);

            ControlResponse {
                status: format!("Shutdown initiated with signal: {signal:?}"),
                uptime: start_time.elapsed().unwrap_or_default(),
                active_connections: active_connections.load(Ordering::Relaxed),
                shutdown_state: *state.lock().unwrap(),
                component: component_name,
            }
        }
        ControlCommand::Status => ControlResponse {
            status: "Service running".to_string(),
            uptime: start_time.elapsed().unwrap_or_default(),
            active_connections: active_connections.load(Ordering::Relaxed),
            shutdown_state: *state.lock().unwrap(),
            component: component_name,
        },
        ControlCommand::Ping => ControlResponse {
            status: "Pong".to_string(),
            uptime: start_time.elapsed().unwrap_or_default(),
            active_connections: active_connections.load(Ordering::Relaxed),
            shutdown_state: *state.lock().unwrap(),
            component: component_name,
        },
        ControlCommand::Reload => ControlResponse {
            status: "Reload not implemented yet".to_string(),
            uptime: start_time.elapsed().unwrap_or_default(),
            active_connections: active_connections.load(Ordering::Relaxed),
            shutdown_state: *state.lock().unwrap(),
            component: component_name,
        },
    };

    let response_json = serde_json::to_string(&response)?;
    let mut stream = reader.into_inner();
    stream.write_all(response_json.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;

    Ok(())
}

/// Get POSIX-compliant default socket path for component.
pub fn get_default_socket_path(component: &str) -> PathBuf {
    // Check environment variable first
    if let Ok(path) = std::env::var("FLEETINGDNS_CONTROL_SOCKET") {
        return PathBuf::from(path);
    }

    // Determine if running as root
    let is_root = unsafe { libc::getuid() == 0 };

    if is_root {
        // System service mode
        #[cfg(target_os = "linux")]
        return PathBuf::from(format!("/run/fleetingdns/{}.sock", component));

        #[cfg(target_os = "macos")]
        return PathBuf::from(format!("/var/run/fleetingdns/{component}.sock"));
    } else {
        // User mode
        #[cfg(target_os = "linux")]
        {
            if let Ok(xdg_runtime) = std::env::var("XDG_RUNTIME_DIR") {
                return PathBuf::from(format!("{}/fleetingdns/{}.sock", xdg_runtime, component));
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(format!(
                    "{home}/Library/Application Support/fleetingdns/{component}.sock"
                ));
            }
        }

        // Fallback to a user-writable location
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(format!("{home}/.local/run/fleetingdns/{component}.sock"));
        }
    }

    // Last resort fallback (should never happen in practice)
    PathBuf::from(format!("/tmp/fleetingdns-{component}.sock"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shutdown_framework_creation() {
        let shutdown = GracefulShutdown::new("test").unwrap();
        assert_eq!(shutdown.state(), ShutdownState::Running);
        assert_eq!(shutdown.active_connections(), 0);
    }

    #[tokio::test]
    async fn test_connection_tracking() {
        let shutdown = GracefulShutdown::new("test").unwrap();

        shutdown.connection_started();
        assert_eq!(shutdown.active_connections(), 1);

        shutdown.connection_started();
        assert_eq!(shutdown.active_connections(), 2);

        shutdown.connection_finished();
        assert_eq!(shutdown.active_connections(), 1);

        shutdown.connection_finished();
        assert_eq!(shutdown.active_connections(), 0);
    }

    #[tokio::test]
    async fn test_shutdown_signal_broadcast() {
        let shutdown = GracefulShutdown::new("test").unwrap();
        let mut rx = shutdown.subscribe();

        shutdown.shutdown(ShutdownSignal::Graceful).await.unwrap();

        let signal = rx.recv().await.unwrap();
        assert_eq!(signal, ShutdownSignal::Graceful);
        assert_eq!(shutdown.state(), ShutdownState::Draining);
    }

    #[test]
    fn test_socket_path_generation() {
        // Test component name formatting
        let path = get_default_socket_path("test-component");
        assert!(path.to_string_lossy().contains("test-component"));

        // Test environment variable override
        unsafe {
            std::env::set_var("FLEETINGDNS_CONTROL_SOCKET", "/custom/path.sock");
        }
        let path = get_default_socket_path("test");
        assert_eq!(path, PathBuf::from("/custom/path.sock"));
        unsafe {
            std::env::remove_var("FLEETINGDNS_CONTROL_SOCKET");
        }
    }
}
