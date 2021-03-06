/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::path::Path;
use std::mem::size_of;
use std::mem::transmute;
use std::mem::zeroed;
use std::os::errno;

use geom::point::TypedPoint2D;

use libc::c_int;
use libc::c_long;
use libc::time_t;
use libc::open;
use libc::read;
use libc::O_RDONLY;

use compositing::windowing::WindowEvent;
use compositing::windowing::ScrollWindowEvent;
use compositing::windowing::ZoomWindowEvent;
use compositing::windowing::MouseWindowMouseDownEvent;
use compositing::windowing::MouseWindowMouseUpEvent;
use compositing::windowing::MouseWindowClickEvent;
use compositing::windowing::MouseWindowEventClass;


extern {
    // XXX: no variadic form in std libs?
    fn ioctl(fd: c_int, req: c_int, ...) -> c_int;
}

#[repr(C)]
struct linux_input_event {
    sec: time_t,
    msec: c_long,
    evt_type: u16,
    code: u16,
    value: i32,
}

#[repr(C)]
struct linux_input_absinfo {
    value: i32,
    minimum: i32,
    maximum: i32,
    fuzz: i32,
    flat: i32,
    resolution: i32,
}

const IOC_NONE: c_int = 0;
const IOC_WRITE: c_int = 1;
const IOC_READ: c_int = 2;

fn ioc(dir: c_int, ioctype: c_int, nr: c_int, size: c_int) -> c_int {
    dir << 30 | size << 16 | ioctype << 8 | nr
}

fn ev_ioc_g_abs(abs: u16) -> c_int {
    ioc(IOC_READ, 'E' as c_int, (0x40 + abs) as i32, size_of::<linux_input_absinfo>() as i32)
}

const EV_SYN: u16 = 0;
const EV_ABS: u16 = 3;

const EV_REPORT: u16 = 0;

const ABS_MT_SLOT: u16 = 0x2F;
const ABS_MT_TOUCH_MAJOR: u16 = 0x30;
const ABS_MT_TOUCH_MINOR: u16 = 0x31;
const ABS_MT_WIDTH_MAJOR: u16 = 0x32;
const ABS_MT_WIDTH_MINOR: u16 = 0x33;
const ABS_MT_ORIENTATION: u16 = 0x34;
const ABS_MT_POSITION_X: u16 = 0x35;
const ABS_MT_POSITION_Y: u16 = 0x36;
const ABS_MT_TRACKING_ID: u16 = 0x39;

struct input_slot {
    tracking_id: i32,
    x: i32,
    y: i32,
}

fn dist(x1: i32, x2: i32, y1: i32, y2: i32) -> f32 {
    let deltaX = (x2 - x1) as f32;
    let deltaY = (y2 - y1) as f32;
    (deltaX * deltaX + deltaY * deltaY).sqrt()
}

fn read_input_device(device_path: &Path,
                     sender: &Sender<Option<WindowEvent>>) {
    // XXX we really want to use std::io:File but it currently doesn't expose
    // the raw FD which is necessary for ioctl.
    let device = unsafe { device_path.as_str().unwrap().with_c_str(|s| open(s, O_RDONLY, 0)) };
    if device == -1 {
        panic!("Couldn't open {}", device_path.as_str().unwrap());
    }

    let mut xInfo: linux_input_absinfo = unsafe { zeroed() };
    let mut yInfo: linux_input_absinfo = unsafe { zeroed() };
    unsafe {
        let ret = ioctl(device, ev_ioc_g_abs(ABS_MT_POSITION_X), &xInfo);
        if ret < 0 {
            println!("Couldn't get ABS_MT_POSITION_X info {} {}", ret, errno());
        }
    }
    unsafe {
        let ret = ioctl(device, ev_ioc_g_abs(ABS_MT_POSITION_Y), &yInfo);
        if ret < 0 {
            println!("Couldn't get ABS_MT_POSITION_Y info {} {}", ret, errno());
        }
    }

    let touchWidth = xInfo.maximum - xInfo.minimum;
    let touchHeight = yInfo.maximum - yInfo.minimum;

    println!("xMin: {}, yMin: {}, touchWidth: {}, touchHeight: {}", xInfo.minimum, yInfo.minimum, touchWidth, touchHeight);

    // XXX: Why isn't size_of treated as constant?
    // let buf: [u8, ..(16 * size_of::<linux_input_event>())];
    let mut buf: [u8, ..(16 * 16)] = unsafe { zeroed() };
    let mut slots: [input_slot, ..10] = unsafe { zeroed() };
    for slot in slots.iter_mut() {
        slot.tracking_id = -1;
    }

    let mut last_x = 0;
    let mut last_y = 0;
    let mut first_x = 0;
    let mut first_y = 0;

    let mut last_dist: f32 = 0f32;
    let mut touch_count: i32 = 0;
    let mut current_slot: uint = 0;
    // XXX: Need to use the real dimensions of the screen
    let screen_dist = dist(0, 480, 854, 0);
    loop {
        let read = unsafe { read(device, transmute(buf.as_mut_ptr()), buf.len() as u32) };

        if read < 0 {
            println!("Couldn't read device! error {}", read);
            return;
        }
        assert!(read % (size_of::<linux_input_event>() as i32) == 0,
                "Unexpected input device read length!");

        let count = read / (size_of::<linux_input_event>() as i32);
        let events: *mut linux_input_event = unsafe { transmute(buf.as_mut_ptr()) };
        let mut tracking_updated = false;
        for idx in range(0, count as int) {
            let event: &linux_input_event = unsafe { transmute(events.offset(idx)) };
            match (event.evt_type, event.code) {
                (EV_SYN, EV_REPORT) => {
                    let slotA = &slots[0];
                    if tracking_updated {
                        tracking_updated = false;
                        if slotA.tracking_id == -1 {
                            println!("Touch up");
                            let delta_x = slotA.x - first_x;
                            let delta_y = slotA.y - first_y;
                            let dist = delta_x * delta_x + delta_y * delta_y;
                            if dist < 16 {
                                let click_pt = TypedPoint2D(slotA.x as f32, slotA.y as f32);
                                println!("Dispatching click!");
                                sender.send(Some(MouseWindowEventClass(MouseWindowMouseDownEvent(0, click_pt))));
                                sender.send(Some(MouseWindowEventClass(MouseWindowMouseUpEvent(0, click_pt))));
                                sender.send(Some(MouseWindowEventClass(MouseWindowClickEvent(0, click_pt))));
                            }
                        } else {
                            println!("Touch down");
                            last_x = slotA.x;
                            last_y = slotA.y;
                            first_x = slotA.x;
                            first_y = slotA.y;
                            if touch_count >= 2 {
                                let slotB = &slots[1];
                                last_dist = dist(slotA.x, slotB.x, slotA.y, slotB.y);
                            }
                        }
                    } else {
                        println!("Touch move x: {}, y: {}", slotA.x, slotA.y);
                        sender.send(Some(ScrollWindowEvent(TypedPoint2D((slotA.x - last_x) as f32, (slotA.y - last_y) as f32), TypedPoint2D(slotA.x, slotA.y))));
                        last_x = slotA.x;
                        last_y = slotA.y;
                        if touch_count >= 2 {
                            let slotB = &slots[1];
                            let cur_dist = dist(slotA.x, slotB.x, slotA.y, slotB.y);
                            println!("Zooming {} {} {} {}", cur_dist, last_dist, screen_dist, ((screen_dist + (cur_dist - last_dist))/screen_dist));
                            sender.send(Some(ZoomWindowEvent((screen_dist + (cur_dist - last_dist))/screen_dist)));
                            last_dist = cur_dist;
                        }
                    }
                },
                (EV_SYN, _) => println!("Unknown SYN code {}", event.code),
                (EV_ABS, ABS_MT_SLOT) => {
                            if (event.value as uint) < slots.len() {
                                current_slot = event.value as uint;
                            } else {
                                println!("Invalid slot! {}", event.value);
                            }
                        },
                (EV_ABS, ABS_MT_TOUCH_MAJOR) => (),
                (EV_ABS, ABS_MT_TOUCH_MINOR) => (),
                (EV_ABS, ABS_MT_WIDTH_MAJOR) => (),
                (EV_ABS, ABS_MT_WIDTH_MINOR) => (),
                (EV_ABS, ABS_MT_ORIENTATION) => (),
                (EV_ABS, ABS_MT_POSITION_X) => {
                    slots[current_slot].x = event.value - xInfo.minimum;
                },
                (EV_ABS, ABS_MT_POSITION_Y) => {
                    slots[current_slot].y = event.value - yInfo.minimum;
                },
                (EV_ABS, ABS_MT_TRACKING_ID) => {
                    let current_id = slots[current_slot].tracking_id;
                    if current_id != event.value &&
                       (current_id == -1 || event.value == -1) {
                        tracking_updated = true;
                        if event.value == -1 {
                            touch_count -= 1;
                        } else {
                            touch_count += 1;
                        }
                    }
                    slots[current_slot].tracking_id = event.value;
                },
                (EV_ABS, _) => println!("Unknown ABS code {}", event.code),
                (_, _) => println!("Unknown event type {}", event.evt_type),
            }
        }
    }
}

pub fn run_input_loop(event_sender: &Sender<Option<WindowEvent>>) {
    let sender = event_sender.clone();
    spawn(proc () {
        // XXX need to scan all devices and read every one.
        let touchinputdev = Path::new("/dev/input/event0");
        read_input_device(&touchinputdev, &sender);
    });
}
