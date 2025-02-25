use nalgebra::{Matrix3, Vector3};

use super::super::constants::{DPI, EARTH_MAJOR_AXIS, EARTH_MINOR_AXIS, ERAU, RADSEC, T2000};
use super::super::ref_system::{nutn80, obleq, rotmt, rotpn};
use hifitime::prelude::Epoch;
use hifitime::ut1::Ut1Provider;

/// Get the observer position and velocity on the Earth 
/// 
/// Argument
/// --------
/// * tmjd: time of the observation in modified julian date (MJD)
/// * longitude: observer longitude on Earth in degree
/// * latitude: observer latitude on Earth in degree
/// * height: observer height on Earth in degree
/// 
/// Return
/// ------
/// * dx: corrected observer position with respect to the center of mass of Earth (in ecliptic J2000)
/// * dy: corrected observer velocity with respect to the center of mass of Earth (in ecliptic J2000)
fn pvobs(tmjd: f64, longitude: f64, latitude: f64, height: f64) -> (Vector3<f64>, Vector3<f64>) {
    // Initialisation
    let omega = Vector3::new(0.0, 0.0, DPI * 1.00273790934);
    let mut dx = Vector3::zeros();
    let mut dv = Vector3::zeros();

    // Get the coordinates of the observer on Earth
    let dxbf = body_fixed_coord(longitude, latitude, height);

    // Get the observer velocity due to Earth rotation
    let dvbf = omega.cross(&dxbf);

    // deviation from Orbfit, use of another conversion from MJD UTC (ET scale) to UT1 scale
    // based on the hifitime crate
    let epoch_mjd = Epoch::from_mjd_utc(tmjd);
    let ut1_provider = Ut1Provider::download_short_from_jpl().unwrap();
    let mjd_ut1 = epoch_mjd.to_ut1(ut1_provider);
    let tut = mjd_ut1.to_mjd_utc_days();

    // Compute the Greenwich sideral apparent time
    let gast = gmst(tut) + equequ(tmjd);

    // Earth rotation matrix
    let rot = rotmt(-gast, 2);

    // Transformation in the ecliptic mean J2000
    let mut rot1 = [[0.; 3]; 3];
    rotpn(&mut rot1, "EQUT", "OFDATE", tmjd, "ECLM", "J2000", 0.);

    let rot1_mat = Matrix3::from(rot1).transpose();
    let rot_mat = Matrix3::from(rot).transpose();

    let rotmat = rot1_mat * rot_mat;

    // Apply transformation to the observer position and velocity
    dx = rotmat * dxbf;
    dv = rotmat * dvbf;

    (dx, dv)
}

/// Compute the Greenwich Mean Sidereal Time (GMST)
/// in radians, for a modified julian date (UT1).
///
/// Arguments
/// ---------
/// * `tjm` - Modified Julian Date (MJD)
///
/// Retour
/// ------
/// GMST in radians, in the [0, 2π) interval.
fn gmst(tjm: f64) -> f64 {
    /// Coefficients du polynôme pour GMST à 0h UT1 (en secondes)
    const C0: f64 = 24110.54841;
    const C1: f64 = 8640184.812866;
    const C2: f64 = 9.3104e-2;
    const C3: f64 = -6.2e-6;
    /// Rapport entre jour sidéral et jour solaire
    const RAP: f64 = 1.00273790934;

    let itjm = tjm.floor();
    let t = (itjm - T2000) / 36525.0;

    // Calcul du temps sidéral moyen à 0h UT1
    let mut gmst0 = ((C3 * t + C2) * t + C1) * t + C0;

    gmst0 *= DPI / 86400.0;

    // Incrément de GMST à partir de 0h
    // let h = (57028.476562500000 - 57028.0) * DPI;
    let h = tjm.fract() * DPI;
    let mut gmst = gmst0 + h * RAP;

    // Ajustement pour rester dans [0, 2π]
    let mut i: i64 = (gmst / DPI).floor() as i64;
    if gmst < 0.0 {
        i = i - 1;
    }
    gmst -= i as f64 * DPI;

    gmst
}

/// Compute the equinox equation
///
/// Arguments
/// ---------
/// * `tjm`: Modified Julian Date (MJD)
///
/// Retour
/// ------
/// * Equinox equation in radians
fn equequ(tjm: f64) -> f64 {
    let oblm = obleq(tjm);
    let (dpsi, _deps) = nutn80(tjm);
    RADSEC * dpsi * oblm.cos()
}

/// Convert latitude and height in parallax coordinates on the Earth
///
/// Argument
/// --------
/// * lat: observer latitude in radians
/// * height: observer height in kilometer
///
/// Return
/// ------
/// * rho_cos_phi: normalized radius of the observer projected on the equatorial plane
/// * rho_sin_phi: normalized radius of the observer projected on the polar axis.
fn lat_alt_to_parallax(lat: f64, height: f64) -> (f64, f64) {
    let axis_ratio = EARTH_MINOR_AXIS / EARTH_MAJOR_AXIS;
    let u = (lat.sin() * axis_ratio).atan2(lat.cos());

    let rho_sin_phi = axis_ratio * u.sin() + (height / EARTH_MAJOR_AXIS) * lat.sin();
    let rho_cos_phi = u.cos() + (height / EARTH_MAJOR_AXIS) * lat.cos();

    (rho_cos_phi, rho_sin_phi)
}

/// Convert latitude in degree and height in parallax coordinate
///
/// Argument
/// --------
/// * lat: observer latitude in degree
/// * height: observer height in kilometer
///
/// Return
/// ------
/// * rho_cos_phi: normalized radius of the observer projected on the equatorial plane
/// * rho_sin_phi: normalized radius of the observer projected on the polar axis.
fn geodetic_to_parallax(lat: f64, height: f64) -> (f64, f64) {
    let latitude_rad = lat.to_radians();

    let (rho_cos_phi, rho_sin_phi) = lat_alt_to_parallax(latitude_rad, height);

    (rho_cos_phi, rho_sin_phi)
}

/// Get the fixed position of an observatory using its geographic coordinates
///
/// Argument
/// --------
/// * longitude: observer longitude in degree
/// * latitude: observer latitude in degree
/// * height: observer height in degree
///
/// Return
/// ------
/// * observer fixed coordinates vector on the Earth (not corrected from Earth motion)
/// * units is AU
fn body_fixed_coord(longitude: f64, latitude: f64, height: f64) -> Vector3<f64> {
    let (pxy1, pz1) = geodetic_to_parallax(latitude, height);
    let lon_radians = longitude.to_radians();

    Vector3::new(
        ERAU * pxy1 * lon_radians.cos(),
        ERAU * pxy1 * lon_radians.sin(),
        ERAU * pz1,
    )
}

#[cfg(test)]
mod observer_pos_tests {

    use super::*;

    #[test]
    fn geodetic_to_parallax_test() {
        /// latitude and height of Pan-STARRS 1, Haleakala
        let (pxy1, pz1) = geodetic_to_parallax(20.707233557, 3067.694);
        assert_eq!(pxy1, 0.9362410003211518);
        assert_eq!(pz1, 0.35154299856304305);
    }

    #[test]
    fn body_fixed_coord_test() {
        /// longitude, latitude and height of Pan-STARRS 1, Haleakala
        let (lon, lat, h) = (203.744090000, 20.707233557, 3067.694);
        let obs_fixed_vector = body_fixed_coord(lon, lat, h);
        assert_eq!(
            obs_fixed_vector,
            Vector3::new(
                -0.00003653799439776371,
                -0.00001607260397528885,
                0.000014988110430544328
            )
        )
    }

    #[test]
    fn test_gmst() {
        let tut = 57028.478514610404;
        let res_gmst = gmst(tut);
        assert_eq!(res_gmst, 4.851925725092499);

        let tut = T2000;
        let res_gmst = gmst(tut);
        assert_eq!(res_gmst, 4.894961212789145);
    }

    #[test]
    fn pvobs_test() {
        let tmjd = 57028.479297592596;
        /// longitude, latitude and height of Pan-STARRS 1, Haleakala
        let (lon, lat, h) = (203.744090000, 20.707233557, 3067.694);

        let (observer_position, observer_velocity) = pvobs(tmjd, lon, lat, h);

        assert_eq!(
            observer_position.as_slice(),
            [
                -2.1029664445055886e-5,
                3.7089965349631534e-5,
                2.911548164794497e-7
            ]
        );
        assert_eq!(
            observer_velocity.as_slice(),
            [
                -0.00021367298085517918,
                -0.00012156695591212987,
                5.304083328775301e-5
            ]
        );
    }
}
