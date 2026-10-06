mod type_conversions {
    use crate::{Quaternion, Vector3};

    #[test]
    fn vector3_round_trips_through_array() {
        let v = Vector3::from([1.0, 2.0, 3.0]);
        assert_eq!(
            v,
            Vector3 {
                x: 1.0,
                y: 2.0,
                z: 3.0
            }
        );
        assert_eq!(<[f32; 3]>::from(v), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn quaternion_round_trips_through_array_scalar_last() {
        let q = Quaternion::from([1.0, 2.0, 3.0, 4.0]);
        assert_eq!(
            q,
            Quaternion {
                x: 1.0,
                y: 2.0,
                z: 3.0,
                w: 4.0
            }
        );
        assert_eq!(<[f32; 4]>::from(q), [1.0, 2.0, 3.0, 4.0]);
    }
}

#[cfg(feature = "mint")]
mod mint_conversions {
    use crate::{Quaternion, Vector3};

    #[test]
    fn vector3_round_trips_through_mint() {
        let v = Vector3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let m: mint::Vector3<f32> = v.into();
        assert_eq!(
            m,
            mint::Vector3 {
                x: 1.0,
                y: 2.0,
                z: 3.0
            }
        );
        assert_eq!(Vector3::from(m), v);
    }

    #[test]
    fn quaternion_round_trips_through_mint_with_w_as_scalar() {
        let q = Quaternion {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            w: 4.0,
        };
        let m: mint::Quaternion<f32> = q.into();
        assert_eq!(
            m,
            mint::Quaternion {
                v: mint::Vector3 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0
                },
                s: 4.0
            }
        );
        assert_eq!(Quaternion::from(m), q);
    }

    #[test]
    fn converts_into_nalgebra_via_mint() {
        let m: mint::Vector3<f32> = Vector3::from([1.0, 2.0, 3.0]).into();
        let v: nalgebra::Vector3<f32> = m.into();
        assert_eq!(v, nalgebra::Vector3::new(1.0, 2.0, 3.0));

        let m: mint::Quaternion<f32> = Quaternion::from([1.0, 2.0, 3.0, 4.0]).into();
        let q: nalgebra::Quaternion<f32> = m.into();
        assert_eq!(q, nalgebra::Quaternion::new(4.0, 1.0, 2.0, 3.0));
    }
}
