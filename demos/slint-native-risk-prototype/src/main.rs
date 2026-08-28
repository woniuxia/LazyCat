mod workspace;

use std::{cell::Cell, rc::Rc, time::Duration};

slint::include_modules!();

fn main() -> anyhow::Result<()> {
    let window = MainWindow::new()?;
    workspace::Workspace::attach(&window)?;

    if std::env::args().any(|argument| argument == "--state-smoke") {
        window.show()?;
        let passed = Rc::new(Cell::new(false));
        let actions = [
            (1000, 0, false, false),
            (2000, 0, true, false),
            (3000, 1, false, false),
            (4000, 0, false, false),
            (5000, 0, false, true),
        ];
        for (delay, tool, set_probe, verify) in actions {
            let weak = window.as_weak();
            let passed = passed.clone();
            slint::Timer::single_shot(Duration::from_millis(delay), move || {
                if let Some(window) = weak.upgrade() {
                    if set_probe {
                        window.invoke_set_active_probe();
                    } else if verify {
                        passed.set(window.invoke_active_probe_matches());
                    } else {
                        window.invoke_open_tool(tool);
                    }
                }
            });
        }
        slint::Timer::single_shot(Duration::from_millis(6000), || {
            slint::quit_event_loop().expect("quit state smoke event loop");
        });
        slint::run_event_loop()?;
        anyhow::ensure!(
            passed.get(),
            "state was not restored into the recreated component"
        );
        return Ok(());
    }

    if std::env::args().any(|argument| argument == "--focus-smoke") {
        window.show()?;
        let weak = window.as_weak();
        for (delay, action) in [
            (100, (Some(0), false)),
            (250, (None, true)),
            (400, (Some(1), false)),
            (550, (None, true)),
            (700, (Some(2), false)),
            (850, (None, true)),
            (1000, (Some(0), false)),
        ] {
            let weak = weak.clone();
            slint::Timer::single_shot(Duration::from_millis(delay), move || {
                if let Some(window) = weak.upgrade() {
                    if let Some(index) = action.0 {
                        window.invoke_open_tool(index);
                    }
                    if action.1 {
                        window.invoke_focus_active_probe();
                    }
                }
            });
        }
        slint::Timer::single_shot(Duration::from_millis(1300), || {
            slint::quit_event_loop().expect("quit focus smoke event loop");
        });
        slint::run_event_loop()?;
        return Ok(());
    }

    if std::env::args().any(|argument| argument == "--startup-smoke") {
        window.show()?;
        let weak = window.as_weak();
        for index in 0..12 {
            let weak = weak.clone();
            slint::Timer::single_shot(Duration::from_millis(100 + index * 50), move || {
                if let Some(window) = weak.upgrade() {
                    window.invoke_open_tool(index as i32);
                }
            });
        }
        slint::Timer::single_shot(Duration::from_millis(800), || {
            slint::quit_event_loop().expect("quit startup smoke event loop");
        });
        slint::run_event_loop()?;
        return Ok(());
    }

    window.run()?;
    Ok(())
}
