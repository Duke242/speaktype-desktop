//! Pausing media through MPRIS, the D-Bus interface that music players,
//! browsers and video players on Linux desktops share.

use zbus::{
    blocking::{Connection, Proxy, fdo::DBusProxy, proxy::Builder},
    proxy::CacheProperties,
};

const PREFIX: &str = "org.mpris.MediaPlayer2.";
/// playerctld relays to whichever player was active last, which is paused directly.
const RELAY: &str = "org.mpris.MediaPlayer2.playerctld";
const PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

/// The players that were playing and got paused, by bus name.
pub struct PausedMedia(Vec<String>);

/// Pauses every player that is playing.
pub fn pause_media() -> Option<PausedMedia> {
    let connection = Connection::session()
        .map_err(|e| eprintln!("[media] no session bus: {e}"))
        .ok()?;
    let names = DBusProxy::new(&connection)
        .map_err(|e| e.to_string())
        .and_then(|bus| bus.list_names().map_err(|e| e.to_string()))
        .map_err(|e| eprintln!("[media] couldn't list players: {e}"))
        .ok()?;
    let paused: Vec<String> = names
        .iter()
        .map(|name| name.to_string())
        .filter(|name| name.starts_with(PREFIX) && name != RELAY)
        .filter(|name| {
            status(&connection, name).as_deref() == Some("Playing")
                && call(&connection, name, "Pause")
        })
        .collect();
    (!paused.is_empty()).then_some(PausedMedia(paused))
}

/// Plays the players `pause_media` paused, unless the user has since stopped,
/// closed or restarted them.
pub fn resume_media(paused: PausedMedia) {
    let Ok(connection) = Connection::session() else {
        return;
    };
    for name in &paused.0 {
        if status(&connection, name).as_deref() == Some("Paused") {
            call(&connection, name, "Play");
        }
    }
}

fn player<'a>(connection: &Connection, name: &'a str) -> zbus::Result<Proxy<'a>> {
    Builder::<Proxy<'a>>::new(connection)
        .destination(name)?
        .path(PATH)?
        .interface(PLAYER)?
        .cache_properties(CacheProperties::No)
        .build()
}

/// "Playing", "Paused" or "Stopped".
fn status(connection: &Connection, name: &str) -> Option<String> {
    player(connection, name)
        .and_then(|player| player.get_property::<String>("PlaybackStatus"))
        .ok()
}

fn call(connection: &Connection, name: &str, method: &str) -> bool {
    let result = player(connection, name).and_then(|player| player.call::<_, _, ()>(method, &()));
    if let Err(e) = &result {
        eprintln!("[media] {name}: {method} failed: {e}");
    }
    result.is_ok()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    struct MockPlayer {
        status: Arc<Mutex<&'static str>>,
    }

    #[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
    impl MockPlayer {
        fn pause(&self) {
            *self.status.lock().unwrap() = "Paused";
        }

        fn play(&self) {
            *self.status.lock().unwrap() = "Playing";
        }

        #[zbus(property)]
        fn playback_status(&self) -> String {
            self.status.lock().unwrap().to_string()
        }
    }

    fn serve(name: &str, status: &'static str) -> (Connection, Arc<Mutex<&'static str>>) {
        let status = Arc::new(Mutex::new(status));
        let player = MockPlayer {
            status: status.clone(),
        };
        let connection = zbus::blocking::connection::Builder::session()
            .and_then(|b| b.name(format!("{PREFIX}{name}")))
            .and_then(|b| b.serve_at(PATH, player))
            .and_then(|b| b.build())
            .expect("serve a mock player");
        (connection, status)
    }

    #[test]
    #[ignore = "needs a D-Bus session bus: dbus-run-session -- cargo test -- --ignored"]
    fn pauses_and_resumes_only_what_was_playing() {
        let (_music, music) = serve("music", "Playing");
        let (_video, video) = serve("video", "Paused");

        let paused = pause_media().expect("music was playing");
        assert_eq!(*music.lock().unwrap(), "Paused");
        assert_eq!(*video.lock().unwrap(), "Paused");
        resume_media(paused);
        assert_eq!(*music.lock().unwrap(), "Playing");
        assert_eq!(*video.lock().unwrap(), "Paused", "the user paused this one");

        // Stopped by the user while dictating: stays stopped.
        let paused = pause_media().expect("music was playing");
        *music.lock().unwrap() = "Stopped";
        resume_media(paused);
        assert_eq!(*music.lock().unwrap(), "Stopped");
        assert!(pause_media().is_none(), "nothing is playing");
    }
}
