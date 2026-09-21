use cgmath::{Deg, InnerSpace, Matrix3, Rad};
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
    is_move_forward_pressed: bool,
    is_move_backward_pressed: bool,
    is_move_left_pressed: bool,
    is_move_right_pressed: bool,
    is_rotate_up_pressed: bool,
    is_rotate_down_pressed: bool,
    is_rotate_left_pressed: bool,
    is_rotate_right_pressed: bool,
}

impl CameraController {
    pub(crate) fn new(speed: f32) -> Self {
        Self {
            speed,
            scroll: 0.0,
            is_locked: false,
            is_move_forward_pressed: false,
            is_move_backward_pressed: false,
            is_move_left_pressed: false,
            is_move_right_pressed: false,
            is_rotate_up_pressed: false,
            is_rotate_down_pressed: false,
            is_rotate_left_pressed: false,
            is_rotate_right_pressed: false,
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
            KeyCode::ArrowUp => {
                self.is_move_forward_pressed = is_pressed;
                true
            }
            KeyCode::ArrowLeft => {
                self.is_move_left_pressed = is_pressed;
                true
            }
            KeyCode::ArrowDown => {
                self.is_move_backward_pressed = is_pressed;
                true
            }
            KeyCode::ArrowRight => {
                self.is_move_right_pressed = is_pressed;
                true
            }

            KeyCode::KeyW => {
                self.is_rotate_up_pressed = is_pressed;
                true
            }
            KeyCode::KeyA => {
                self.is_rotate_left_pressed = is_pressed;
                true
            }
            KeyCode::KeyS => {
                self.is_rotate_down_pressed = is_pressed;
                true
            }
            KeyCode::KeyD => {
                self.is_rotate_right_pressed = is_pressed;
                true
            }

            KeyCode::Digit2 => {
                if is_pressed {
                    self.is_locked = !self.is_locked;
                    if self.is_locked {
                        camera.lock_to_ground();
                    } else {
                        camera.unlock();
                    }
                }
                true
            }
            _ => false,
        }
    }

    pub(crate) fn update_camera(&mut self, camera: &mut Camera) {
        let move_speed = self.speed * 0.15;
        let rotate_speed = 0.003;

        let view = camera.target - camera.eye;
        let view_dir = view.normalize();
        let right = view_dir.cross(camera.up).normalize();

        let mut movement = cgmath::Vector3::new(0.0, 0.0, 0.0);

        if self.is_rotate_up_pressed {
            movement += view_dir * move_speed;
        }
        if self.is_rotate_down_pressed {
            movement -= view_dir * move_speed;
        }
        if self.is_rotate_right_pressed {
            movement += right * move_speed;
        }
        if self.is_rotate_left_pressed {
            movement -= right * move_speed;
        }

        if movement.magnitude2() > 0.0 {
            camera.eye += movement;
            camera.target += movement;
        }

        let mut offset = camera.eye - camera.target;

        if self.is_move_left_pressed {
            let rotation = Matrix3::from_angle_y(Rad(rotate_speed));
            offset = rotation * offset;
        }
        if self.is_move_right_pressed {
            let rotation = Matrix3::from_angle_y(Rad(-rotate_speed));
            offset = rotation * offset;
        }

        let orbit_right = offset.normalize().cross(camera.up).normalize();

        if self.is_move_forward_pressed {
            let rotation = Matrix3::from_axis_angle(orbit_right, Rad(rotate_speed));
            offset = rotation * offset;
        }
        if self.is_move_backward_pressed {
            let rotation = Matrix3::from_axis_angle(orbit_right, Rad(-rotate_speed));
            offset = rotation * offset;
        }

        camera.eye = camera.target + offset;

        if self.scroll.abs() > 0.0 {
            let zoom_dir = (camera.target - camera.eye).normalize();
            camera.eye += zoom_dir * self.scroll * self.speed;
            self.scroll = 0.0;
        }
    }
}
