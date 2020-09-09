use nalgebra::Vector2;
use specs::{Component, DenseVecStorage, FlaggedStorage};

use crate::{
    nalgebra::{Isometry2, Point2, RealField},
    nphysics::{
        algebra::{Force2, ForceType, Velocity2},
        object::{Body, BodyPart, BodyStatus, DefaultBodyHandle, RigidBody, RigidBodyDesc},
    },
};

pub mod util {
    use specs::{Component, DenseVecStorage, FlaggedStorage};

    use crate::{
        bodies::Position,
        nalgebra::{Isometry2, RealField},
    };
    use nalgebra::Vector2;

    pub struct SimplePosition<N: RealField>(pub Isometry2<N>);

    impl<N: RealField> Position<N> for SimplePosition<N> {
        fn isometry(&self) -> &Isometry2<N> {
            &self.0
        }

        fn isometry_mut(&mut self) -> &mut Isometry2<N> {
            &mut self.0
        }

        fn set_isometry(&mut self, isometry: &Isometry2<N>) -> &mut Self {
            self.0.rotation = isometry.rotation;
            self.0.translation = isometry.translation;
            self
        }
    }

    impl<N: RealField> Component for SimplePosition<N> {
        type Storage = FlaggedStorage<Self, DenseVecStorage<Self>>;
    }

    impl<N: RealField> Default for SimplePosition<N> {
        fn default() -> Self {
            SimplePosition {
                0: Isometry2::new(
                    Vector2::new(
                        N::from_f32(0.0).unwrap(),
                        N::from_f32(0.0).unwrap(),
                    ),
                    N::from_f32(0.0).unwrap()),
            }
        }
    }
}

/// An implementation of the `Position` trait is required for the
/// synchronisation of the position of Specs and nphysics objects.
///
/// Initially, it is used to position bodies in the nphysics `World`. Then after
/// progressing the `World` it is used to synchronise the updated positions back
/// towards Specs.
pub trait Position<N: RealField>:
    Component<Storage = FlaggedStorage<Self, DenseVecStorage<Self>>> + Send + Sync
{
    fn isometry(&self) -> &Isometry2<N>;
    fn isometry_mut(&mut self) -> &mut Isometry2<N>;
    fn set_isometry(&mut self, isometry: &Isometry2<N>) -> &mut Self;
}


/// The `PhysicsBody` `Component` represents a `PhysicsWorld` `RigidBody` in
/// Specs and contains all the data required for the synchronisation between
/// both worlds.
#[derive(Clone, Copy, Debug)]
pub struct PhysicsBody<N: RealField> {
    pub(crate) handle: Option<DefaultBodyHandle>,
    pub gravity_enabled: bool,
    pub body_status: BodyStatus,
    pub velocity: Velocity2<N>,
    pub angular_inertia: N,
    pub mass: N,
    pub local_center_of_mass: Point2<N>,
    pub rotations_kinematic: bool,
    external_forces: Force2<N>,
    pub translations_kinematic: Vector2<bool>,
}

impl<N: RealField> Component for PhysicsBody<N> {
    type Storage = FlaggedStorage<Self, DenseVecStorage<Self>>;
}

impl<N: RealField> PhysicsBody<N> {
    pub fn check_external_force(&self) -> &Force2<N> {
        &self.external_forces
    }

    pub fn apply_external_force(&mut self, force: &Force2<N>) -> &mut Self {
        self.external_forces += *force;
        self
    }

    /// For creating new rigid body from this component's values
    pub(crate) fn to_rigid_body_desc(&self) -> RigidBodyDesc<N> {
        RigidBodyDesc::new()
            .gravity_enabled(self.gravity_enabled)
            .status(self.body_status)
            .velocity(self.velocity)
            .angular_inertia(self.angular_inertia)
            .mass(self.mass)
            .local_center_of_mass(self.local_center_of_mass)
    }

    /// Note: applies forces by draining external force property
    pub(crate) fn apply_to_physics_world(&mut self, rigid_body: &mut RigidBody<N>) -> &mut Self {
        rigid_body.enable_gravity(self.gravity_enabled);
        rigid_body.set_status(self.body_status);
        rigid_body.set_velocity(self.velocity);
        rigid_body.set_angular_inertia(self.angular_inertia);
        rigid_body.set_mass(self.mass);
        rigid_body.set_local_center_of_mass(self.local_center_of_mass);
        rigid_body.apply_force(0, &self.drain_external_force(), ForceType::Force, true);
        rigid_body.set_rotations_kinematic(self.rotations_kinematic);
        rigid_body.set_translations_kinematic(self.translations_kinematic);
        self
    }

    pub(crate) fn update_from_physics_world(&mut self, rigid_body: &RigidBody<N>) -> &mut Self {
        // These two probably won't be modified but hey
        self.gravity_enabled = rigid_body.gravity_enabled();
        self.body_status = rigid_body.status();

        self.velocity = *rigid_body.velocity();

        let local_inertia = rigid_body.local_inertia();
        self.angular_inertia = local_inertia.angular;
        self.mass = local_inertia.linear;
        self
    }

    fn drain_external_force(&mut self) -> Force2<N> {
        let value = self.external_forces;
        self.external_forces = Force2::<N>::zero();
        value
    }
}

/// The `PhysicsBodyBuilder` implements the builder pattern for `PhysicsBody`s
/// and is the recommended way of instantiating and customising new
/// `PhysicsBody` instances.
///
/// # Example
///
/// ```rust
/// use specs_physics::{
///     nalgebra::{Matrix2, Point2},
///     nphysics::{algebra::Velocity2, object::BodyStatus},
///     PhysicsBodyBuilder,
/// };
///
/// let physics_body = PhysicsBodyBuilder::from(BodyStatus::Dynamic)
///     .gravity_enabled(true)
///     .velocity(Velocity2::linear(1.0, 1.0))
///     .angular_inertia(3.0)
///     .mass(1.3)
///     .local_center_of_mass(Point2::new(0.0, 0.0))
///     .build();
/// ```
pub struct PhysicsBodyBuilder<N: RealField> {
    gravity_enabled: bool,
    body_status: BodyStatus,
    velocity: Velocity2<N>,
    angular_inertia: N,
    mass: N,
    local_center_of_mass: Point2<N>,
    rotations_kinematic: bool,
    translations_kinematic: Vector2<bool>,

}

impl<N: RealField> From<BodyStatus> for PhysicsBodyBuilder<N> {
    /// Creates a new `PhysicsBodyBuilder` from the given `BodyStatus`. This
    /// also populates the `PhysicsBody` with sane defaults.
    fn from(body_status: BodyStatus) -> Self {
        Self {
            gravity_enabled: false,
            body_status,
            velocity: Velocity2::zero(),
            angular_inertia: N::from_f32(0.0).unwrap(),
            mass: N::from_f32(1.2).unwrap(),
            local_center_of_mass: Point2::origin(),
            rotations_kinematic: false,
            translations_kinematic: Vector2::new(false, false),
        }
    }
}

impl<N: RealField> PhysicsBodyBuilder<N> {
    /// Sets the `gravity_enabled` value of the `PhysicsBodyBuilder`.
    pub fn gravity_enabled(mut self, gravity_enabled: bool) -> Self {
        self.gravity_enabled = gravity_enabled;
        self
    }

    // Sets the `velocity` value of the `PhysicsBodyBuilder`.
    pub fn velocity(mut self, velocity: Velocity2<N>) -> Self {
        self.velocity = velocity;
        self
    }

    /// Sets the `angular_inertia` value of the `PhysicsBodyBuilder`.
    pub fn angular_inertia(mut self, angular_inertia: N) -> Self {
        self.angular_inertia = angular_inertia;
        self
    }

    /// Sets the `mass` value of the `PhysicsBodyBuilder`.
    pub fn mass(mut self, mass: N) -> Self {
        self.mass = mass;
        self
    }

    /// Sets the `local_center_of_mass` value of the `PhysicsBodyBuilder`.
    pub fn local_center_of_mass(mut self, local_center_of_mass: Point2<N>) -> Self {
        self.local_center_of_mass = local_center_of_mass;
        self
    }

    pub fn rotations_kinematic(mut self, rotations_kinematic: bool) -> Self {
        self.rotations_kinematic = rotations_kinematic;
        self
    }

    pub fn lock_rotations(mut self, lock_rotations: bool) -> Self {
        self.rotations_kinematic = lock_rotations;
        self
    }

    pub fn translations_kinematic(mut self, translations_kinematic: Vector2<bool>) -> Self {
        self.translations_kinematic = translations_kinematic;
        self
    }

    pub fn lock_translations(mut self, lock_translations: Vector2<bool>) -> Self {
        self.translations_kinematic = lock_translations;
        self
    }
    /// Builds the `PhysicsBody` from the values set in the `PhysicsBodyBuilder`
    /// instance.
    pub fn build(self) -> PhysicsBody<N> {
        PhysicsBody {
            handle: None,
            gravity_enabled: self.gravity_enabled,
            body_status: self.body_status,
            velocity: self.velocity,
            angular_inertia: self.angular_inertia,
            mass: self.mass,
            local_center_of_mass: self.local_center_of_mass,
            external_forces: Force2::zero(),
            rotations_kinematic: self.rotations_kinematic,
            translations_kinematic: self.translations_kinematic,
        }
    }
}
