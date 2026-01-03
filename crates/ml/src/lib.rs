use ndarray::Array1;
use ort::session::Session;
use std::path::Path;
use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum MlError {
    #[error("ONNX runtime error: {0}")]
    OrtError(#[from] ort::Error),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Invalid input shape")]
    InvalidShape,
}

pub struct ArbitragePredictor {
    session: Session,
    scaler_mean: Array1<f32>,
    scaler_scale: Array1<f32>,
    n_features: usize,
}

impl ArbitragePredictor {
    pub fn load<P: AsRef<Path>>(model_path: P, scaler_mean: Vec<f32>, scaler_scale: Vec<f32>) -> Result<Self, MlError> {
        info!("Loading ONNX model from {:?}", model_path.as_ref());
        
        let n_features = scaler_mean.len();
        
        let session = Session::builder()?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)?
            .commit_from_file(model_path)?;
        
        Ok(Self {
            session,
            scaler_mean: Array1::from_vec(scaler_mean),
            scaler_scale: Array1::from_vec(scaler_scale),
            n_features,
        })
    }
    
    fn scale_features(&self, features: &[f32]) -> Result<Vec<f32>, MlError> {
        if features.len() != self.n_features {
            return Err(MlError::InvalidShape);
        }
        
        Ok(features
            .iter()
            .zip(self.scaler_mean.iter())
            .zip(self.scaler_scale.iter())
            .map(|((x, mean), scale)| (x - mean) / scale)
            .collect())
    }
    
    pub fn predict(&mut self, features: &[f32]) -> Result<f32, MlError> {
        let scaled = self.scale_features(features)?;
        let shape = [1_usize, self.n_features];
        
        let input_tensor = ort::value::Tensor::from_array((shape, scaled.into_boxed_slice()))?;
        
        let outputs = self.session.run(ort::inputs![input_tensor])?;
        
        let output_tensor = &outputs[0];
        let (_, data) = output_tensor.try_extract_tensor::<f32>()?;
        
        Ok(data[0])
    }
    
    pub fn predict_batch(&mut self, features: &[Vec<f32>]) -> Result<Vec<f32>, MlError> {
        if features.is_empty() {
            return Ok(vec![]);
        }
        
        let n_samples = features.len();
        
        let mut scaled = Vec::with_capacity(n_samples * self.n_features);
        for sample in features {
            scaled.extend(self.scale_features(sample)?);
        }
        
        let shape = [n_samples, self.n_features];
        let input_tensor = ort::value::Tensor::from_array((shape, scaled.into_boxed_slice()))?;
        
        let outputs = self.session.run(ort::inputs![input_tensor])?;
        
        let output_tensor = &outputs[0];
        let (_, data) = output_tensor.try_extract_tensor::<f32>()?;
        
        Ok(data.to_vec())
    }
    
    pub fn is_profitable(&mut self, features: &[f32], threshold: f32) -> Result<bool, MlError> {
        Ok(self.predict(features)? > threshold)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArbitrageFeatures {
    pub spread_binance_coinbase: f32,
    pub spread_binance_kraken: f32,
    pub spread_coinbase_kraken: f32,
    pub volume_binance: f32,
    pub volume_coinbase: f32,
    pub volume_kraken: f32,
    pub volatility: f32,
    pub hour_of_day: f32,
    pub day_of_week: f32,
    pub liquidity_score: f32,
    pub max_spread_bps: f32,
}

impl ArbitrageFeatures {
    pub fn to_vec(&self) -> Vec<f32> {
        vec![
            self.spread_binance_coinbase,
            self.spread_binance_kraken,
            self.spread_coinbase_kraken,
            self.volume_binance,
            self.volume_coinbase,
            self.volume_kraken,
            self.volatility,
            self.hour_of_day,
            self.day_of_week,
            self.liquidity_score,
            self.max_spread_bps,
        ]
    }
}

pub mod prelude {
    pub use crate::{ArbitrageFeatures, ArbitragePredictor, MlError};
}
