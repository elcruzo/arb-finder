#!/usr/bin/env python3
import sys; sys.stdout.reconfigure(line_buffering=True)
import pandas as pd
import numpy as np
import joblib
import torch
from torch.utils.data import DataLoader, TensorDataset
from pathlib import Path
from sklearn.model_selection import train_test_split
from sklearn.metrics import accuracy_score, f1_score, roc_auc_score

from model import create_model

MODELS_DIR = Path('models')
scaler = joblib.load(MODELS_DIR / 'scaler.joblib')
with open(MODELS_DIR / 'feature_cols.txt') as f:
    FEATURE_COLS = f.read().strip().split('\n')

print('Loading data...', flush=True)
df = pd.read_csv('data/arbitrage_training_data.csv')
X = df[FEATURE_COLS].values
y = df['is_profitable'].values
X_train, X_test, y_train, y_test = train_test_split(X, y, test_size=0.2, random_state=42, stratify=y)

X_train_scaled = scaler.transform(X_train)
X_test_scaled = scaler.transform(X_test)

device = torch.device('mps') if torch.backends.mps.is_available() else torch.device('cpu')
print(f'Device: {device}', flush=True)

X_t = torch.from_numpy(X_train_scaled.astype(np.float32))
y_t = torch.from_numpy(y_train.astype(np.float32)).unsqueeze(1)
loader = DataLoader(TensorDataset(X_t, y_t), batch_size=512, shuffle=True)

model = create_model(len(FEATURE_COLS)).to(device)
print(f'Parameters: {sum(p.numel() for p in model.parameters()):,}', flush=True)

opt = torch.optim.AdamW(model.parameters(), lr=0.001, weight_decay=1e-4)
scheduler = torch.optim.lr_scheduler.ReduceLROnPlateau(opt, patience=5, factor=0.5)
loss_fn = torch.nn.BCELoss()

print('Training...', flush=True)
best_loss = float('inf')
for epoch in range(100):
    model.train()
    total = 0
    for bx, by in loader:
        bx, by = bx.to(device), by.to(device)
        opt.zero_grad()
        loss = loss_fn(model(bx), by)
        loss.backward()
        opt.step()
        total += loss.item()
    
    avg_loss = total / len(loader)
    scheduler.step(avg_loss)
    
    if avg_loss < best_loss:
        best_loss = avg_loss
    
    if (epoch + 1) % 10 == 0:
        print(f'Epoch {epoch+1}/100 - Loss: {avg_loss:.4f}', flush=True)

model.eval()
with torch.no_grad():
    X_test_t = torch.from_numpy(X_test_scaled.astype(np.float32)).to(device)
    y_prob_nn = model(X_test_t).cpu().numpy().flatten()

y_pred_nn = (y_prob_nn > 0.5).astype(int)
print(f'Accuracy: {accuracy_score(y_test, y_pred_nn):.4f}', flush=True)
print(f'F1: {f1_score(y_test, y_pred_nn):.4f}', flush=True)
print(f'AUC: {roc_auc_score(y_test, y_prob_nn):.4f}', flush=True)

torch.save({
    'model_state_dict': model.state_dict(),
    'input_size': len(FEATURE_COLS),
    'feature_cols': FEATURE_COLS
}, MODELS_DIR / 'arbitrage_net.pth')
print('Saved: arbitrage_net.pth', flush=True)
