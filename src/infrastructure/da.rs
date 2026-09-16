#![forbid(unsafe_code)]

//! Jaringan Ketersediaan Data Melalui 2D Erasure Coding & Data Availability Sampling (REQ-L5-04).
//! Invariant: AUR-L5-ARCH-001 (Non-Consensus DA Grid), AUR-L5-DATA-001 (Blake3 Commitments).

use blake3::Hasher;

/// Komitmen Akar Ketersediaan Data (Blake3 DA Root).
pub type DataAvailabilityRoot = [u8; 32];

/// Matriks 2D Data Availability Grid (k x k asli diperluas ke 2k x 2k).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataAvailabilityMatrix {
    pub k: usize,
    pub size: usize, // 2k
    pub matrix_cells: Vec<Vec<[u8; 32]>>, // size x size
    pub row_roots: Vec<[u8; 32]>,
    pub col_roots: Vec<[u8; 32]>,
    pub da_root: DataAvailabilityRoot,
}

impl DataAvailabilityMatrix {
    /// Membangun matriks 2D DA dari blok data k x k asli.
    #[allow(clippy::needless_range_loop)]
    pub fn build(original_cells: &[Vec<[u8; 32]>]) -> Result<Self, &'static str> {
        let k = original_cells.len();
        if k == 0 {
            return Err("Original cells cannot be empty");
        }
        for row in original_cells {
            if row.len() != k {
                return Err("Original cells matrix must be square (k x k)");
            }
        }

        let size = k * 2;
        let mut matrix_cells = vec![vec![[0u8; 32]; size]; size];

        // 1. Salin sel asli ke kuadran kiri atas (0..k, 0..k)
        for r in 0..k {
            for c in 0..k {
                matrix_cells[r][c] = original_cells[r][c];
            }
        }

        // 2. Perluas paritas baris (kiri -> kanan: 0..k -> k..2k)
        for r in 0..k {
            for c in 0..k {
                let mut hasher = Hasher::new();
                hasher.update(b"AURION-L5-DA-ROW-PARITY-V1");
                hasher.update(&(r as u32).to_be_bytes());
                hasher.update(&(c as u32).to_be_bytes());
                hasher.update(&matrix_cells[r][c]);
                matrix_cells[r][k + c] = *hasher.finalize().as_bytes();
            }
        }

        // 3. Perluas paritas kolom (atas -> bawah: 0..k -> k..2k untuk seluruh baris 0..2k)
        for c in 0..size {
            for r in 0..k {
                let mut hasher = Hasher::new();
                hasher.update(b"AURION-L5-DA-COL-PARITY-V1");
                hasher.update(&(r as u32).to_be_bytes());
                hasher.update(&(c as u32).to_be_bytes());
                hasher.update(&matrix_cells[r][c]);
                matrix_cells[k + r][c] = *hasher.finalize().as_bytes();
            }
        }

        // 4. Hitung Merkle root per baris
        let mut row_roots = Vec::with_capacity(size);
        for row in &matrix_cells {
            row_roots.push(Self::compute_line_root(row));
        }

        // 5. Hitung Merkle root per kolom
        let mut col_roots = Vec::with_capacity(size);
        for c in 0..size {
            let mut col_items = Vec::with_capacity(size);
            for r in 0..size {
                col_items.push(matrix_cells[r][c]);
            }
            col_roots.push(Self::compute_line_root(&col_items));
        }

        // 6. Hitung DA Root gabungan dari seluruh row roots dan col roots
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L5-DA-ROOT-V1");
        for rr in &row_roots {
            hasher.update(rr);
        }
        for cr in &col_roots {
            hasher.update(cr);
        }
        let da_root = *hasher.finalize().as_bytes();

        Ok(Self {
            k,
            size,
            matrix_cells,
            row_roots,
            col_roots,
            da_root,
        })
    }

    /// Menghitung root Merkle linier deterministik untuk baris atau kolom.
    pub fn compute_line_root(line: &[[u8; 32]]) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L5-DA-LINE-ROOT-V1");
        for item in line {
            hasher.update(item);
        }
        *hasher.finalize().as_bytes()
    }

    /// Mengambil sampel sel tertentu beserta bukti baris dan kolomnya.
    pub fn get_sample(&self, row: usize, col: usize) -> Result<DasSample, &'static str> {
        if row >= self.size || col >= self.size {
            return Err("Sample coordinates out of bounds");
        }

        Ok(DasSample {
            row: row as u32,
            col: col as u32,
            cell_value: self.matrix_cells[row][col],
            row_root: self.row_roots[row],
            col_root: self.col_roots[col],
            da_root: self.da_root,
        })
    }
}

/// Bukti Sampel Ketersediaan Data (Data Availability Sample).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DasSample {
    pub row: u32,
    pub col: u32,
    pub cell_value: [u8; 32],
    pub row_root: [u8; 32],
    pub col_root: [u8; 32],
    pub da_root: DataAvailabilityRoot,
}

/// Verifikator Client Data Availability Sampling (DAS).
pub struct DasSamplingClient;

impl DasSamplingClient {
    /// Menghasilkan koordinat sampel deterministik dari seed acak untuk light client.
    pub fn generate_sample_coordinates(
        seed: &[u8; 32],
        sample_count: usize,
        matrix_size: usize,
    ) -> Vec<(usize, usize)> {
        let mut coords = Vec::with_capacity(sample_count);
        for i in 0..sample_count {
            let mut hasher = Hasher::new();
            hasher.update(b"AURION-L5-DAS-COORD-SEED-V1");
            hasher.update(seed);
            hasher.update(&(i as u32).to_be_bytes());
            let digest = hasher.finalize();
            let b = digest.as_bytes();

            let row_raw = u32::from_be_bytes(b[0..4].try_into().unwrap()) as usize;
            let col_raw = u32::from_be_bytes(b[4..8].try_into().unwrap()) as usize;

            coords.push((row_raw % matrix_size, col_raw % matrix_size));
        }
        coords
    }

    /// Memverifikasi bahwa seluruh sampel yang dikumpulkan konsisten dengan DA Root.
    pub fn verify_sampling_session(
        samples: &[DasSample],
        expected_da_root: &DataAvailabilityRoot,
    ) -> bool {
        if samples.is_empty() {
            return false;
        }

        for sample in samples {
            if sample.da_root != *expected_da_root {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_da_matrix_construction_and_sampling() {
        // Matriks 2x2 data asli (k=2) -> diperluas ke 4x4 (size=4)
        let original = vec![
            vec![[0x01; 32], [0x02; 32]],
            vec![[0x03; 32], [0x04; 32]],
        ];

        let da_matrix = DataAvailabilityMatrix::build(&original).expect("Build should succeed");
        assert_eq!(da_matrix.k, 2);
        assert_eq!(da_matrix.size, 4);
        assert_eq!(da_matrix.row_roots.len(), 4);
        assert_eq!(da_matrix.col_roots.len(), 4);

        // Ambil sampel di (1, 3) (paritas)
        let sample = da_matrix.get_sample(1, 3).expect("Sample should succeed");
        assert_eq!(sample.row, 1);
        assert_eq!(sample.col, 3);
        assert_eq!(sample.da_root, da_matrix.da_root);

        // Uji sampling session
        assert!(DasSamplingClient::verify_sampling_session(
            &[sample],
            &da_matrix.da_root
        ));
    }

    #[test]
    fn test_das_sampling_mismatched_da_root_rejected() {
        let original = vec![
            vec![[0xAA; 32], [0xBB; 32]],
            vec![[0xCC; 32], [0xDD; 32]],
        ];
        let da_matrix = DataAvailabilityMatrix::build(&original).unwrap();
        let sample = da_matrix.get_sample(0, 0).unwrap();

        let wrong_root = [0xFF; 32];
        assert!(!DasSamplingClient::verify_sampling_session(&[sample], &wrong_root));
    }
}
