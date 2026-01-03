#!/usr/bin/env python3
"""Train all models by running XGBoost and PyTorch in separate processes."""
import subprocess
import sys

print('=== Training XGBoost ===')
subprocess.run([sys.executable, 'scripts/train_xgb.py'], check=True)

print('\n=== Training PyTorch NN ===')
subprocess.run([sys.executable, 'scripts/train_nn.py'], check=True)

print('\n=== All models trained! ===')
