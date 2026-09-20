use cgmath::InnerSpace;
use winit::event::MouseScrollDelta;
use winit::keyboard::KeyCode;

pub struct Camera {
    pub eye: cgmath::Point3<f32>,
    pub target: cgmath::Point3<f32>,
    pub up: cgmath::Vector3<f32>,
    pub aspect: f32,
    pub fov_y: f32,
    pub z_near: f32,
    pub z_far: f32,
}

impl Camera {
    fn build_view_projection_matrix(&self) -> cgmath::Matrix4<f32> {
        let view = cgmath::Matrix4::look_at_rh(self.eye, self.target, self.up);
        let proj = cgmath::perspective(
            cgmath::Deg(self.fov_y),
            self.aspect,
            self.z_near,
            self.z_far,
        );
        return OPENGL_TO_WGPU_MATRIX * proj * view;
    }
    pub fn lock_to_ground(&mut self) {
        let height = (self.eye.y - self.target.y).abs().max(2.0);
        self.eye = cgmath::Point3::new(self.target.x, self.target.y + height, self.target.z);
        self.up = -cgmath::Vector3::unit_z();
    }
    pub fn unlock(&mut self) {
        self.up = cgmath::Vector3::unit_y();
    }
}

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::from_cols(
    cgmath::Vector4::new(1.0, 0.0, 0.0, 0.0),
    cgmath::Vector4::new(0.0, 1.0, 0.0, 0.0),
    cgmath::Vector4::new(0.0, 0.0, 0.5, 0.0),
    cgmath::Vector4::new(0.0, 0.0, 0.5, 1.0),
);

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    pub fn new() -> Self {
        use cgmath::SquareMatrix;
        Self {
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera) {
        self.view_proj = camera.build_view_projection_matrix().into();
    }
}

pub struct CameraController {
    speed: f32,
    scroll: f32,
    pub is_locked: bool,
    is_forward_pressed: bool,
    is_backward_pressed: bool,
    is_left_pressed: bool,
    is_right_pressed: bool,
}

impl CameraController {
    pub(crate) fn new(speed: f32) -> Self {
        Self {
            speed,
            scroll: 0.0,
            is_locked: false,
            is_forward_pressed: false,
            is_backward_pressed: false,
            is_left_pressed: false,
            is_right_pressed: false,
        }
    }
    pub(crate) fn handle_scroll(&mut self, camera: &mut Camera, delta: &MouseScrollDelta) {
        let delta_y = match delta {
            MouseScrollDelta::LineDelta(_, y) => *y * 0.5,
            MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.01,
        };
        if self.is_locked {
            camera.eye.y = (camera.eye.y - delta_y * self.speed).clamp(2.0, 100.0);
        } else {
            self.scroll = delta_y * 0.5;
        }
    }
    pub(crate) fn handle_key(
        &mut self,
        camera: &mut Camera,
        code: KeyCode,
        is_pressed: bool,
    ) -> bool {
        match code {
            KeyCode::KeyW | KeyCode::ArrowUp => {
                self.is_forward_pressed = is_pressed;
                true
            }
            KeyCode::KeyA | KeyCode::ArrowLeft => {
                self.is_left_pressed = is_pressed;
                true
            }
            KeyCode::KeyS | KeyCode::ArrowDown => {
                self.is_backward_pressed = is_pressed;
                true
            }
            KeyCode::KeyD | KeyCode::ArrowRight => {
                self.is_right_pressed = is_pressed;
                true
            }
            KeyCode::Digit2 => {
                self.is_locked = !self.is_locked;
                if self.is_locked {
                    camera.lock_to_ground();
                } else {
                    camera.unlock();
                }
                true
            }
            _ => false,
        }
    }

    pub(crate) fn update_camera(&self, camera: &mut Camera) {
        let forward = camera.target - camera.eye;
        let forward_norm = forward.normalize();
        let forward_mag = forward.magnitude();
        if self.is_locked {
            if self.is_forward_pressed {
                camera.eye.z -= self.speed;
                camera.target.z -= self.speed;
            }
            if self.is_backward_pressed {
                camera.eye.z += self.speed;
                camera.target.z += self.speed;
            }
            if self.is_left_pressed {
                camera.eye.x -= self.speed;
                camera.target.x -= self.speed;
            }
            if self.is_right_pressed {
                camera.eye.x += self.speed;
                camera.target.x += self.speed;
            }
        } else {
            if self.is_forward_pressed && forward_mag > self.speed {
                camera.eye += forward_norm * self.speed;
            }
            if self.is_backward_pressed {
                camera.eye -= forward_norm * self.speed;
            }
            if self.scroll.abs() > 0.0 {
                camera.eye += forward_norm * self.scroll * self.speed;
            }
            let right = forward_norm.cross(camera.up);
            let forward = camera.target - camera.eye;
            let forward_mag = forward.magnitude();
            if self.is_right_pressed {
                camera.eye =
                    camera.target - (forward + right * self.speed).normalize() * forward_mag;
            }
            if self.is_left_pressed {
                camera.eye =
                    camera.target - (forward - right * self.speed).normalize() * forward_mag;
            }
        }
    }
}
