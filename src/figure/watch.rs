use super::{Figure, FigureError, Rendered};

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, SystemTime};

/// How often the file is looked at.
const POLL: Duration = Duration::from_millis(400);
/// Editors often save in two steps (truncate, then write); give the second a moment to land.
const SETTLE: Duration = Duration::from_millis(80);

/// Every time the figure at `path` is saved, load it and render it at `scale` on a
/// background thread, and send the result — a new figure, or why it could not be used.
///
/// Polls the modification time rather than subscribing to file events: it needs no extra
/// dependency, works the same everywhere, and survives editors that save by replacing the
/// file. The thread ends when the receiver is dropped.
pub fn watch(path: PathBuf, scale: f32) -> Receiver<Result<Rendered, FigureError>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut seen = modified(&path);
        loop {
            thread::sleep(POLL);
            let now = modified(&path);
            if now == seen {
                continue;
            }
            thread::sleep(SETTLE);
            seen = modified(&path);
            if tx.send(Figure::load(&path).map(|f| f.render(scale))).is_err() {
                return;
            }
        }
    });
    rx
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const FIGURE: &str = r##"{
        canvas: [200, 200], center: [100, 100], background: "#000000", ink: "#FFFFFF",
        loop: { frames: 10, fps: 10 },
        layers: [ { name: "a", turns: 1, elements: [ { type: "circle", r: RADIUS } ] } ],
    }"##;

    /// A good save arrives as a new figure, a broken one as an error saying what is wrong.
    #[test]
    fn saves_arrive_as_figures_or_errors() {
        let dir = std::env::temp_dir().join(format!("imagespin-watch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("figure.json5");
        std::fs::write(&path, FIGURE.replace("RADIUS", "40")).unwrap();
        let rx = watch(path.clone(), 1.0);
        let wait = Duration::from_secs(5);

        // Some filesystems keep modification times to the second; make sure it moves.
        std::thread::sleep(Duration::from_millis(1100));
        std::fs::write(&path, FIGURE.replace("RADIUS", "60")).unwrap();
        let fig = rx.recv_timeout(wait).expect("no reload").expect("valid figure rejected");
        assert!((fig.layers[0].extent[1] - 60.8).abs() < 0.1);

        std::thread::sleep(Duration::from_millis(1100));
        std::fs::write(&path, FIGURE.replace("RADIUS", "-5")).unwrap();
        let err = rx.recv_timeout(wait).expect("no reload").err().expect("broken figure accepted");
        assert!(err.to_string().contains("`r` must be greater than 0"), "{err}");

        std::fs::remove_dir_all(&dir).ok();
    }
}
