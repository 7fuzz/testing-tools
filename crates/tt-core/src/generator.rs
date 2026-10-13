use anyhow::Result;
use image::RgbaImage;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// ChaCha8 rather than `StdRng`: its stream is fixed across `rand` releases, so saved seeds keep reproducing.
pub type Rng = ChaCha8Rng;

pub fn rng(seed: u64) -> Rng {
    Rng::seed_from_u64(seed)
}

pub fn random_seed() -> u64 {
    rand::random()
}

/// An image generator. The same params and seed always give the same image; params fields left unset
/// are filled from the seed.
pub trait Generator: 'static {
    const ID: &'static str;
    const NAME: &'static str;
    type Params: Clone + Default + Serialize + DeserializeOwned;

    fn randomize(rng: &mut Rng) -> Self::Params;
    fn generate(params: &Self::Params, seed: u64) -> Result<RgbaImage>;
}

/// A `Generator` with its params as JSON, so tools that list generators (VCam's Add menu, saved scenes)
/// can hold any of them without knowing the concrete types.
#[derive(Clone, Copy)]
pub struct ErasedGenerator {
    pub id: &'static str,
    pub name: &'static str,
    pub randomize: fn(seed: u64) -> Value,
    pub generate: fn(params: &Value, seed: u64) -> Result<RgbaImage>,
}

impl ErasedGenerator {
    pub const fn of<G: Generator>() -> Self {
        Self { id: G::ID, name: G::NAME, randomize: randomize_json::<G>, generate: generate_json::<G> }
    }
}

pub fn to_json<T: Serialize>(params: &T) -> Value {
    serde_json::to_value(params).expect("generator params serialize to JSON")
}

fn randomize_json<G: Generator>(seed: u64) -> Value {
    to_json(&G::randomize(&mut rng(seed)))
}

fn generate_json<G: Generator>(params: &Value, seed: u64) -> Result<RgbaImage> {
    let params: G::Params = serde_json::from_value(params.clone())?;
    G::generate(&params, seed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng as _;
    use serde::Deserialize;

    struct Swatch;

    #[derive(Clone, Default, Serialize, Deserialize)]
    #[serde(default)]
    struct SwatchParams {
        rgb: Option<[u8; 3]>,
        size: u32,
    }

    impl Generator for Swatch {
        const ID: &'static str = "swatch";
        const NAME: &'static str = "Swatch";
        type Params = SwatchParams;

        fn randomize(rng: &mut Rng) -> SwatchParams {
            SwatchParams { rgb: Some(rng.random()), size: rng.random_range(1..8) }
        }

        fn generate(params: &SwatchParams, seed: u64) -> Result<RgbaImage> {
            let [r, g, b] = params.rgb.unwrap_or_else(|| rng(seed).random());
            Ok(RgbaImage::from_pixel(params.size.max(1), params.size.max(1), image::Rgba([r, g, b, 255])))
        }
    }

    #[test]
    fn same_seed_gives_same_output() {
        let erased = ErasedGenerator::of::<Swatch>();
        assert_eq!((erased.randomize)(7), (erased.randomize)(7));
        assert_ne!((erased.randomize)(7), (erased.randomize)(8));

        let unset = to_json(&SwatchParams { rgb: None, size: 2 });
        let a = (erased.generate)(&unset, 42).unwrap();
        let b = (erased.generate)(&unset, 42).unwrap();
        assert_eq!(a.as_raw(), b.as_raw());
    }

    #[test]
    fn missing_json_fields_fall_back_to_defaults() {
        let erased = ErasedGenerator::of::<Swatch>();
        let image = (erased.generate)(&serde_json::json!({ "rgb": [1, 2, 3] }), 0).unwrap();
        assert_eq!(image.dimensions(), (1, 1));
        assert_eq!(image.get_pixel(0, 0).0, [1, 2, 3, 255]);
    }
}
