use crate::camera::Camera;
use cgmath::{Deg, Vector2};
use winit::event::MouseButton;

pub struct CameraController {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,

    rmb: bool,
    prev_cursor: Option<Vector2<f32>>,
    current_cursor: Option<Vector2<f32>>,

    move_speed: f32,
    rotation_speed: f32,
}

impl CameraController {
    pub fn new(move_speed: f32, rotation_speed: f32) -> Self {
        Self {
            move_speed,
            rotation_speed,
            forward: false,
            back: false,
            left: false,
            rmb: false,
            prev_cursor: None,
            current_cursor: None,
            right: false,
        }
    }

    pub fn process_events(&mut self, event: &winit::event::WindowEvent) -> bool {
        use winit::event::{ElementState, KeyEvent, TouchPhase, WindowEvent};
        use winit::keyboard::{KeyCode, PhysicalKey};

        match event {
            WindowEvent::Touch(touch) => {
                match touch.phase {
                    TouchPhase::Started => {
                        self.rmb = true;
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        self.rmb = false;
                        self.prev_cursor = None;
                        self.current_cursor = None;
                    }
                    TouchPhase::Moved => {
                        self.prev_cursor = self.current_cursor;
                        self.current_cursor = Some(Vector2::new(
                            touch.location.x as f32,
                            touch.location.y as f32,
                        ));
                    }
                }
                true
            }
            WindowEvent::CursorMoved {
                device_id: _,
                position,
            } => {
                self.prev_cursor = self.current_cursor;
                self.current_cursor = Some(Vector2::new(position.x as f32, position.y as f32));
                false
            }
            WindowEvent::MouseInput {
                device_id: _,
                state,
                button,
            } => {
                if *button == MouseButton::Right {
                    self.rmb = state.is_pressed();
                    true
                } else {
                    false
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state,
                        physical_key: PhysicalKey::Code(keycode),
                        ..
                    },
                ..
            } => {
                let k = *state == ElementState::Pressed;
                match keycode {
                    KeyCode::KeyW | KeyCode::ArrowUp => {
                        self.forward = k;
                        true
                    }
                    KeyCode::KeyA | KeyCode::ArrowLeft => {
                        self.left = k;
                        true
                    }
                    KeyCode::KeyS | KeyCode::ArrowDown => {
                        self.back = k;
                        true
                    }
                    KeyCode::KeyD | KeyCode::ArrowRight => {
                        self.right = k;
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    pub fn update_camera(&mut self, camera: &mut Camera) {
        match (self.rmb, self.prev_cursor, self.current_cursor) {
            (true, Some(prev), Some(curr)) => {
                let delta = (curr - prev) * self.rotation_speed;
                let mut r = *camera.get_rotator();
                r.yaw += Deg(delta.x);
                r.pitch += Deg(delta.y);
                camera.set_rotator(r);
                self.prev_cursor = None;
            }
            _ => {}
        };

        let mut forward = 0;
        let mut right = 0;

        if self.forward {
            forward += 1
        }
        if self.back {
            forward -= 1
        }
        if self.left {
            right += 1
        }
        if self.right {
            right -= 1
        }

        if forward != 0 || right != 0 {
            camera.set_eye(
                camera.get_eye()
                    + camera.forward() * (forward as f32) * self.move_speed
                    + camera.right() * (right as f32) * self.move_speed,
            );
        }
    }
}
