#!/usr/bin/env python3 -u
"""Export trained models to ONNX format for Rust inference."""

import sys
sys.stdout.reconfigure(line_buffering=True)

import json
import numpy as np
import joblib
import torch
from pathlib import Path

from model import create_model

MODELS_DIR = Path('models')

print('Exporting models to ONNX...\n')

with open(MODELS_DIR / 'feature_cols.txt') as f:
    feature_cols = f.read().strip().split('\n')

INPUT_SIZE = len(feature_cols)

print('Loading PyTorch model...')
ckpt = torch.load(MODELS_DIR / 'arbitrage_net.pth', weights_only=False)
model = create_model(INPUT_SIZE)
model.load_state_dict(ckpt['model_state_dict'])
model.eval()

print('Exporting PyTorch to ONNX...')
dummy_input = torch.randn(1, INPUT_SIZE)
torch.onnx.export(
    model,
    dummy_input,
    MODELS_DIR / 'arbitrage_net.onnx',
    input_names=['features'],
    output_names=['probability'],
    dynamic_axes={'features': {0: 'batch'}, 'probability': {0: 'batch'}},
    opset_version=17
)
print(f'  Saved: arbitrage_net.onnx')

print('\nExporting XGBoost to native format...')
xgb_clf = joblib.load(MODELS_DIR / 'xgboost_classifier.joblib')
xgb_clf.save_model(MODELS_DIR / 'xgboost_classifier.json')
print(f'  Saved: xgboost_classifier.json')

print('\nExporting scaler parameters...')
scaler = joblib.load(MODELS_DIR / 'scaler.joblib')
params = {'mean': scaler.mean_.tolist(), 'scale': scaler.scale_.tolist()}
with open(MODELS_DIR / 'scaler_params.json', 'w') as f:
    json.dump(params, f)
print(f'  Saved: scaler_params.json')

print('\n=== Export Complete ===')
for f in sorted(MODELS_DIR.iterdir()):
    if f.is_file():
        print(f'  {f.name} ({f.stat().st_size / 1024:.1f} KB)')
