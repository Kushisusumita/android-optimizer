//! Speed up only: caches and compilation, no removals. The one to run after a system update.

use super::snapshot::Snapshot;
use super::{steps, Session};
use crate::tune::cleanup;
use crate::ui::chart::Progress;

pub fn run(s: &Session, before: &Snapshot) {
    let launch = steps::measure_speed(s);
    let mut progress = Progress::new(3);
    progress.next("Cleaning caches");
    cleanup::clean_caches(&s.phone.adb);
    progress.next("Compiling apps");
    steps::compile(s, &before.diag, s.opts.no_compile.then_some("--no-compile"));
    progress.next("Measuring the result");
    steps::finish(s, before, launch);
}
