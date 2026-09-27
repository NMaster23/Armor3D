use cgmath::{InnerSpace, Matrix3, Rad};
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
    pub orthographic: bool,
}

impl Camera {
    pub fn build_view_projection_matrix(&self) -> cgmath::Matrix4<f32> {
        let view = cgmath::Matrix4::look_at_rh(self.eye, self.target, self.up);
        let proj = if self.orthographic {
            let half_height = (self.eye - self.target).magnitude()
                * (self.fov_y.to_radians() * 0.5).tan();
            let half_width = half_height * self.aspect;
            cgmath::ortho(-half_width, half_width, -half_height, half_height, self.z_near, self.z_far)
        } else {
            cgmath::perspective(cgmath::Deg(self.fov_y), self.aspect, self.z_near, self.z_far)
        };
        return OPENGL_TO_WGPU_MATRIX * proj * view;
    }
    pub fn lock_to_ground(&mut self) {
        let height = (self.eye - self.target).magnitude().max(2.0);
        self.eye = cgmath::Point3::new(self.target.x, self.target.y + height, self.target.z);
        self.up = -cgmath::Vector3::unit_z();
        self.orthographic = true;
    }
    pub fn unlock(&mut self) {
        self.up = cgmath::Vector3::unit_y();
        self.orthographic = false;
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        let forward = (self.target - self.eye).normalize();
        let right = forward.cross(self.up).normalize();
        let up = right.cross(forward).normalize();
        let scale = (self.eye - self.target).magnitude() * 0.002;
        let movement = (-right * dx + up * dy) * scale;
        self.eye += movement;
        self.target += movement;
    }

    pub fn orbit(&mut self, dx: f32, dy: f32) {
        if self.orthographic {
            return;
        }
        let mut offset = self.eye - self.target;
        offset = Matrix3::from_angle_y(Rad(-dx * 0.005)) * offset;
        let forward = (-offset).normalize();
        let right = forward.cross(self.up).normalize();
        let pitched = Matrix3::from_axis_angle(right, Rad(dy * 0.005)) * offset;
        if pitched.normalize().dot(self.up.normalize()).abs() < 0.995 {
            offset = pitched;
        }
        self.eye = self.target + offset;
    }

    pub fn zoom(&mut self, steps: f32) {
        let offset = self.eye - self.target;
        let distance = (offset.magnitude() * 0.9_f32.powf(steps)).clamp(0.2, 80.0);
        self.eye = self.target + offset.normalize() * distance;
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
    saved_3d_view: Option<(cgmath::Point3<f32>, cgmath::Point3<f32>, cgmath::Vector3<f32>)>,
    two_pressed: bool,
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
    pub fn unlock_to_3d(&mut self, camera: &mut Camera) {
        if !self.is_locked {
            return;
        }
        self.is_locked = false;
        camera.unlock();
        let dist = (camera.eye - camera.target).magnitude().max(2.0);
        camera.eye = camera.target + cgmath::Vector3::new(0.0, dist * 0.707, dist * 0.707);
    }
    pub fn new(speed: f32) -> Self {
        Self {
            speed,
            scroll: 0.0,
            is_locked: false,
            saved_3d_view: None,
            two_pressed: false,
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
    pub fn handle_key(
        &mut self,
        camera: &mut Camera,
        code: KeyCode,
        is_pressed: bool,
    ) -> bool {
        if is_pressed && self.is_locked {
            match code {
                KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::ArrowLeft | KeyCode::ArrowRight => {
                    self.unlock_to_3d(camera);
                }
                _ => {}
            }
        }
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
                if is_pressed && !self.two_pressed {
                    self.is_locked = !self.is_locked;
                    if self.is_locked {
                        self.saved_3d_view = Some((camera.eye, camera.target, camera.up));
                        camera.lock_to_ground();
                    } else {
                        camera.unlock();
                        if let Some((eye, target, up)) = self.saved_3d_view.take() {
                            camera.eye = eye;
                            camera.target = target;
                            camera.up = up;
                        }
                    }
                }
                self.two_pressed = is_pressed;
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

        if self.is_locked {
            let screen_up = right.cross(view_dir).normalize();
            let mut movement = cgmath::Vector3::new(0.0, 0.0, 0.0);
            if self.is_rotate_up_pressed || self.is_move_forward_pressed {
                movement += screen_up;
            }
            if self.is_rotate_down_pressed || self.is_move_backward_pressed {
                movement -= screen_up;
            }
            if self.is_rotate_right_pressed || self.is_move_right_pressed {
                movement += right;
            }
            if self.is_rotate_left_pressed || self.is_move_left_pressed {
                movement -= right;
            }
            if movement.magnitude2() > 0.0 {
                let step = movement.normalize() * (self.speed * 0.5);
                camera.eye += step;
                camera.target += step;
            }
            return;
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn camera() -> Camera {
        Camera {
            eye: (0.0, 1.0, 2.0).into(),
            target: (0.0, 0.0, 0.0).into(),
            up: cgmath::Vector3::unit_y(),
            aspect: 1.0,
            fov_y: 45.0,
            z_near: 0.1,
            z_far: 100.0,
            orthographic: false,
        }
    }

    #[test]
    fn pan_moves_eye_and_target_together() {
        let mut camera = camera();
        let offset = camera.eye - camera.target;
        camera.pan(100.0, 50.0);
        assert_ne!(camera.target, cgmath::Point3::new(0.0, 0.0, 0.0));
        assert!(((camera.eye - camera.target) - offset).magnitude() < 0.00001);
    }

    #[test]
    fn orbit_changes_angle_without_changing_distance() {
        let mut camera = camera();
        let eye = camera.eye;
        let distance = (camera.eye - camera.target).magnitude();
        camera.orbit(100.0, 50.0);
        assert_ne!(camera.eye, eye);
        assert!(((camera.eye - camera.target).magnitude() - distance).abs() < 0.00001);
    }

    #[test]
    fn zoom_changes_distance_and_stays_bounded() {
        let mut camera = camera();
        let distance = (camera.eye - camera.target).magnitude();
        camera.zoom(1.0);
        assert!((camera.eye - camera.target).magnitude() < distance);
        camera.zoom(-1000.0);
        assert!((camera.eye - camera.target).magnitude() <= 80.0);
    }

    #[test]
    fn key_press_moves_until_release() {
        let mut camera = camera();
        let mut controller = CameraController::new(0.02);
        controller.handle_key(&mut camera, KeyCode::KeyW, true);
        controller.update_camera(&mut camera);
        let moved = camera.eye;
        controller.handle_key(&mut camera, KeyCode::KeyW, false);
        controller.update_camera(&mut camera);
        assert_ne!(moved, cgmath::Point3::new(0.0, 1.0, 2.0));
        assert_eq!(camera.eye, moved);
    }

    #[test]
    fn two_toggles_top_view_and_restores_3d_pose() {
        let mut camera = camera();
        let eye = camera.eye;
        let target = camera.target;
        let up = camera.up;
        let mut controller = CameraController::new(0.02);

        controller.handle_key(&mut camera, KeyCode::Digit2, true);
        assert!(controller.is_locked);
        assert!(camera.orthographic);
        assert_eq!(camera.eye.x, camera.target.x);
        assert_eq!(camera.eye.z, camera.target.z);
        controller.handle_key(&mut camera, KeyCode::Digit2, true);
        assert!(camera.orthographic);

        let top_eye = camera.eye;
        camera.orbit(100.0, 50.0);
        assert_eq!(camera.eye, top_eye);
        camera.zoom(1.0);
        assert!((camera.eye - camera.target).magnitude() < (top_eye - camera.target).magnitude());

        controller.handle_key(&mut camera, KeyCode::Digit2, false);
        controller.handle_key(&mut camera, KeyCode::Digit2, true);
        assert!(!controller.is_locked);
        assert!(!camera.orthographic);
        assert_eq!(camera.eye, eye);
        assert_eq!(camera.target, target);
        assert_eq!(camera.up, up);
    }
}
