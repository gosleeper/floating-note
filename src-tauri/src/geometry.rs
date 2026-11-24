use crate::storage::{Config, ScreenPosition};
use serde::Serialize;
use tauri::{LogicalSize, Monitor, PhysicalPosition, WebviewWindow};

#[derive(Clone, Copy, Serialize)]
pub struct NoteSize {
    pub width: u32,
    pub height: u32,
}

pub fn fit(window: &WebviewWindow, width: i32, height: u32) -> tauri::Result<NoteSize> {
    let monitor = window.current_monitor()?.or(window.primary_monitor()?);
    let size = if let Some(monitor) = &monitor {
        let scale = monitor.scale_factor();
        let work = monitor.work_area().size.to_logical::<u32>(scale);
        NoteSize {
            width: (width as u32).min(work.width.max(1)),
            height: height.clamp(1, (work.height * 4 / 5).max(1)),
        }
    } else {
        NoteSize {
            width: width as u32,
            height: height.clamp(1, 640),
        }
    };
    let target = LogicalSize::new(size.width, size.height);
    if window.inner_size()? != target.to_physical::<u32>(window.scale_factor()?) {
        window.set_size(target)?;
    }
    if let Some(monitor) = &monitor {
        let position = window.outer_position()?;
        move_inside(window, monitor, position)?;
    }
    Ok(size)
}

fn move_inside(
    window: &WebviewWindow,
    monitor: &Monitor,
    desired: PhysicalPosition<i32>,
) -> tauri::Result<()> {
    let work = monitor.work_area();
    let window_size = window.outer_size()?;
    let max_x = work.position.x + work.size.width.saturating_sub(window_size.width) as i32;
    let max_y = work.position.y + work.size.height.saturating_sub(window_size.height) as i32;
    let position = PhysicalPosition::new(
        desired.x.clamp(work.position.x, max_x),
        desired.y.clamp(work.position.y, max_y),
    );
    if window.outer_position()? != position {
        window.set_position(position)?;
    }
    Ok(())
}

pub fn capture(window: &WebviewWindow) -> tauri::Result<Option<ScreenPosition>> {
    let Some(monitor) = window.current_monitor()? else {
        return Ok(None);
    };
    let origin = monitor.work_area().position;
    let actual = window.outer_position()?;
    let relative = PhysicalPosition::new(actual.x - origin.x, actual.y - origin.y)
        .to_logical::<i32>(monitor.scale_factor());
    Ok(Some(ScreenPosition {
        name: monitor.name().cloned(),
        origin_x: origin.x,
        origin_y: origin.y,
        x: relative.x,
        y: relative.y,
    }))
}

pub fn restore(window: &WebviewWindow, config: &Config, reset: bool) -> tauri::Result<()> {
    let monitors = window.available_monitors()?;
    let mut target = None;
    if !reset {
        if let Some(saved) = &config.position {
            let exact = monitors.iter().find(|monitor| {
                let origin = monitor.work_area().position;
                monitor.name() == saved.name.as_ref()
                    && (origin.x, origin.y) == (saved.origin_x, saved.origin_y)
            });
            let by_name = || {
                let name = saved.name.as_ref()?;
                let mut named = monitors
                    .iter()
                    .filter(|monitor| monitor.name() == Some(name));
                let monitor = named.next()?;
                named.next().is_none().then_some(monitor)
            };
            if let Some(monitor) = exact.or_else(by_name) {
                let origin = monitor.work_area().position;
                let scale = monitor.scale_factor();
                let x = (f64::from(saved.x) * scale) as i32;
                let y = (f64::from(saved.y) * scale) as i32;
                target = Some((
                    monitor,
                    PhysicalPosition::new(origin.x.saturating_add(x), origin.y.saturating_add(y)),
                ));
            }
        } else if let Some((x, y)) = config.x.zip(config.y) {
            // FloatNote 0.2 stored desktop coordinates in logical pixels.
            target = monitors.iter().find_map(|monitor| {
                let scale = monitor.scale_factor();
                let work = monitor.work_area();
                let point = PhysicalPosition::new(
                    (f64::from(x) * scale) as i32,
                    (f64::from(y) * scale) as i32,
                );
                in_work_area(point, work.position, work.size.width, work.size.height)
                    .then_some((monitor, point))
            });
        }
    }
    if let Some((monitor, point)) = target {
        return move_inside(window, monitor, point);
    }

    let monitor = if reset {
        window.current_monitor()?.or(window.primary_monitor()?)
    } else {
        window.primary_monitor()?.or(window.current_monitor()?)
    };
    if let Some(monitor) = monitor {
        let work = monitor.work_area();
        let window_width = window.outer_size()?.width;
        let scale = monitor.scale_factor();
        let point = PhysicalPosition::new(
            work.position.x + work.size.width.saturating_sub(window_width) as i32
                - (60.0 * scale) as i32,
            work.position.y + (100.0 * scale) as i32,
        );
        move_inside(window, &monitor, point)?;
    }
    Ok(())
}

fn in_work_area(
    point: PhysicalPosition<i32>,
    origin: PhysicalPosition<i32>,
    width: u32,
    height: u32,
) -> bool {
    (0..i64::from(width)).contains(&(i64::from(point.x) - i64::from(origin.x)))
        && (0..i64::from(height)).contains(&(i64::from(point.y) - i64::from(origin.y)))
}

pub fn recover_if_offscreen(window: &WebviewWindow, config: &Config) -> tauri::Result<()> {
    let point = window.outer_position()?;
    if !window.available_monitors()?.iter().any(|monitor| {
        let work = monitor.work_area();
        in_work_area(point, work.position, work.size.width, work.size.height)
    }) {
        restore(window, config, false)?;
    }
    Ok(())
}
