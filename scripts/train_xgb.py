#!/usr/bin/env python3
import sys; sys.stdout.reconfigure(line_buffering=True)
import pandas as pd
import numpy as np
import joblib
from pathlib import Path
from sklearn.model_selection import train_test_split
from sklearn.preprocessing import StandardScaler
from sklearn.metrics import accuracy_score, f1_score, roc_auc_score
import xgboost as xgb

MODELS_DIR = Path('models')
MODELS_DIR.mkdir(exist_ok=True)

FEATURE_COLS = [
    'spread_binance_coinbase', 'spread_binance_kraken', 'spread_coinbase_kraken',
    'volume_binance', 'volume_coinbase', 'volume_kraken',
    'volatility', 'hour_of_day', 'day_of_week', 'liquidity_score', 'max_spread_bps'
]

print('Loading data...', flush=True)
df = pd.read_csv('data/arbitrage_training_data.csv')
X = df[FEATURE_COLS].values
y = df['is_profitable'].values
X_train, X_test, y_train, y_test = train_test_split(X, y, test_size=0.2, random_state=42, stratify=y)

scaler = StandardScaler()
X_train_scaled = scaler.fit_transform(X_train)
X_test_scaled = scaler.transform(X_test)

joblib.dump(scaler, MODELS_DIR / 'scaler.joblib')
with open(MODELS_DIR / 'feature_cols.txt', 'w') as f:
    f.write('\n'.join(FEATURE_COLS))

print('Training XGBoost...', flush=True)
xgb_clf = xgb.XGBClassifier(n_estimators=200, max_depth=8, learning_rate=0.1, random_state=42, eval_metric='logloss')
xgb_clf.fit(X_train, y_train, verbose=False)

y_pred = xgb_clf.predict(X_test)
y_prob = xgb_clf.predict_proba(X_test)[:, 1]
print(f'Accuracy: {accuracy_score(y_test, y_pred):.4f}', flush=True)
print(f'F1: {f1_score(y_test, y_pred):.4f}', flush=True)
print(f'AUC: {roc_auc_score(y_test, y_prob):.4f}', flush=True)
joblib.dump(xgb_clf, MODELS_DIR / 'xgboost_classifier.joblib')
print('Saved: xgboost_classifier.joblib', flush=True)

