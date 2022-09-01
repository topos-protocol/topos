use super::cyclerng::{thread_rng, Uniform};

#[allow(unused)]
use rand::distributions::Distribution;

#[derive(Debug, Eq, PartialEq)]
pub enum SamplerError {
    ZeroLength,
    ShortOfInput,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Sample<T: Clone> {
    pub src_len: usize,
    pub sample_size: usize,
    pub value: Vec<T>,
}

#[allow(dead_code)]
pub fn sample_from<T>(src: &Vec<T>) -> Result<Sample<T>, SamplerError>
where
    T: Clone + std::fmt::Debug + Ord,
{
    sample_reduce_from(src, |len| (len as f64).sqrt() as usize)
}

pub fn sample_reduce_from<T, F>(src: &Vec<T>, reducer: F) -> Result<Sample<T>, SamplerError>
where
    T: Clone + std::fmt::Debug + Ord,
    F: Fn(usize) -> usize,
{
    if src.is_empty() {
        return Err(SamplerError::ZeroLength);
    }

    let src_len = src.len();
    let sample_size = reducer(src_len);

    if sample_size == 0 {
        return Err(SamplerError::ZeroLength);
    }

    if sample_size > src_len {
        return Err(SamplerError::ShortOfInput);
    }

    let mut result = Sample {
        src_len,
        sample_size,
        value: Vec::with_capacity(sample_size),
    };
    // Borrow the src mutably so we can sort it
    let mut src = src.to_owned();
    src.sort();
    // Setup the sampling using our rng and a uniform selection
    let mut rng = thread_rng();

    // WHy use uniform sampling over simple modulo based
    // https://docs.rs/rand/0.8.5/rand/distributions/uniform/struct.Uniform.html
    let dist = Uniform::new(0, src_len); // Uniform::new is exclusive of the upper
                                         // Track used entries to ensure we don't double select
    let mut used = vec![false; src_len];
    loop {
        // Sample a value from the uniform distribution
        let mut idx = dist.sample(&mut rng);
        // Find the next unused 'slot'.
        // Usable when sample_size is close to input_len
        while used[idx] {
            idx += 1;
            if idx >= src_len {
                idx = 0;
            }
        }
        used[idx] = true;
        // Push the used value into the result array
        result.value.push(src[idx].to_owned());
        if result.value.len() == sample_size {
            break;
        }
    }
    Ok(result)
}

#[cfg(test)]
mod should {
    use super::{super::cyclerng, thread_rng, Uniform, *};

    #[test]
    fn return_cyclerng_for_samples_in_test() {
        cyclerng::utils::set_cycle([1, 2, 3]);

        let mut rng = thread_rng();

        // Uniform(UniformInt { low: 0, range: 10, z: 6 })
        let dist = Uniform::new(0, 10);

        let _value = dist.sample(&mut rng);
        let value = dist.sample(&mut rng);

        assert_eq!(value, 2usize);
    }

    #[test]
    fn return_zero_length_error() {
        let all = Vec::<u64>::new();

        let error = sample_from(&all).unwrap_err();

        assert_eq!(SamplerError::ZeroLength, error);
    }

    #[test]
    fn sample_correct_entries_based_on_input_size() {
        cyclerng::utils::set_cycle([2, 4, 6]);

        let all = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];

        let actual = sample_from(&all).unwrap();

        let expected = vec![3, 5, 7];

        assert_eq!(expected, actual.value, "{:?}", actual);
    }

    #[test]
    fn sample_correct_entries_with_overlapping_hits() {
        cyclerng::utils::set_cycle([2, 2, 4, 2, 6]);

        let all = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];

        let actual = sample_from(&all).unwrap();

        let expected = vec![3, 4, 5];

        assert_eq!(expected, actual.value, "{:?}", actual);
    }
}
