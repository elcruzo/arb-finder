"""Shared model architecture for training and export."""

import torch.nn as nn

def create_model(input_size: int) -> nn.Module:
    """Create the arbitrage prediction model.
    
    Architecture: MLP with batch normalization and dropout.
    Input: 11 features (spreads, volumes, market conditions)
    Output: probability of profitable arbitrage opportunity
    """
    return nn.Sequential(
        # Input layer
        nn.Linear(input_size, 128),
        nn.BatchNorm1d(128),
        nn.ReLU(),
        nn.Dropout(0.3),
        
        # Hidden layer 1
        nn.Linear(128, 64),
        nn.BatchNorm1d(64),
        nn.ReLU(),
        nn.Dropout(0.2),
        
        # Hidden layer 2
        nn.Linear(64, 32),
        nn.BatchNorm1d(32),
        nn.ReLU(),
        
        # Output layer
        nn.Linear(32, 1),
        nn.Sigmoid()
    )

