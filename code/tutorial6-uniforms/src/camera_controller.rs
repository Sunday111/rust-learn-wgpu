use crate::Camera;
use winit::event::MouseButton;

pub struct CameraController {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,

    speed: f32,
}

impl CameraController {
    pub fn new(speed: f32) -> Self {
        Self {
            speed,
            forward: false,
            back: false,
            left: false,
            right: false,
        }
    }

    pub fn process_events(&mut self, event: &winit::event::WindowEvent) -> bool {
        use winit::event::{ElementState, KeyEvent, WindowEvent};
        use winit::keyboard::{KeyCode, PhysicalKey};

        match event {
            WindowEvent::Focused(focused) => false,
            WindowEvent::CursorMoved {
                device_id,
                position,
            } => false,
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => {
                if *button == MouseButton::Right {
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

    pub fn update_camera(&self, camera: &mut Camera) {
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
                    + camera.forward() * (forward as f32) * self.speed
                    + camera.right() * (right as f32) * self.speed,
            );
            println!("eye: {:?}", camera.get_eye());
        }
    }
}
