use std::ops::{Index, IndexMut};

struct Array2d<T> {
    shape: (usize, usize),
    data: Vec<T>,
}

impl<T> Array2d<T> {
    pub fn new(shape: (usize, usize), data: Vec<T>) -> Self {
        return Self { shape, data };
    }

    pub fn from_vec(d: Vec<Vec<T>>) -> Self {
        if let Some(r) = d.first() {
            let (m, n) = (d.len(), r.len());
            if !d.iter().all(|r| r.len() == n) {
                panic!("inner vectors have different lengths")
            }

            return Self::new((m, n), d.into_iter().flat_map(|r| r.into_iter()).collect());
        }
        return Self::new((0, 0), Vec::new());
    }

    pub fn from_array<const M: usize, const N: usize>(d: [[T; N]; M]) -> Self {
        return Self::new((M, N), d.into_iter().flat_map(|x| x.into_iter()).collect());
    }

    pub fn get(&self, i: usize, j: usize) -> &T {
        return self.data.get(i * self.shape.1 + j).expect("out of bound");
    }

    pub fn get_mut(&mut self, i: usize, j: usize) -> &mut T {
        return self.data.get_mut(i * self.shape.1 + j).expect("out of bound");
    }
}

impl<T> Index<(usize, usize)> for Array2d<T> {
    type Output = T;

    fn index(&self, index: (usize, usize)) -> &Self::Output {
        return self.get(index.0, index.1);
    }
}

impl<T> IndexMut<(usize, usize)> for Array2d<T> {
    fn index_mut(&mut self, index: (usize, usize)) -> &mut Self::Output {
        return self.get_mut(index.0, index.1);
    }
}
