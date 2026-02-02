# Non-Obvious Correlation Discovery: Techniques and Implementation

**Research Date:** 2026-02-02
**Research Focus:** Finding hidden relationships between disparate data sources for predictive signals
**Application Domain:** Financial markets, alternative data, cross-asset relationships
**Status:** Research Complete

---

## Executive Summary

This research catalogs techniques for discovering non-obvious correlations between data sources that predict market movements. The focus is on relationships that are:
1. Not widely known or exploited
2. Have economic rationale (not spurious)
3. Persist over time (not overfit)
4. Provide lead time for positioning

### Key Findings

| Technique Category | Recommended Approach | Computation Cost | Discovery Potential |
|-------------------|---------------------|------------------|---------------------|
| **Statistical Causality** | Granger Causality + Transfer Entropy | Medium | High |
| **Information Theory** | Mutual Information + Copulas | Medium-High | Very High |
| **Time-Lagged Analysis** | Dynamic Time Warping | High | High |
| **ML Feature Discovery** | SHAP + Attention Analysis | High | Very High |
| **Causal Discovery** | NOTEARS + PC Algorithm | Very High | High |
| **Network Analysis** | Correlation Graphs + Community Detection | Medium | Medium-High |

### Critical Insight

> **The best non-obvious correlations combine statistical significance with economic intuition.** Pure data mining without fundamental reasoning leads to spurious relationships that break down out-of-sample.

---

## 1. Statistical Correlation Discovery Methods

### 1.1 Granger Causality Testing

**Description:** Tests whether past values of one time series help predict future values of another, beyond what the second series predicts itself.

**Mathematical Foundation:**
```
H0: X does not Granger-cause Y
H1: X Granger-causes Y

Test: Compare VAR models:
  Restricted:  Y_t = a0 + a1*Y_{t-1} + ... + ap*Y_{t-p} + e_t
  Unrestricted: Y_t = a0 + a1*Y_{t-1} + ... + ap*Y_{t-p} + b1*X_{t-1} + ... + bq*X_{t-q} + e_t

F-statistic compares residual sum of squares
```

**Implementation (Python/statsmodels):**
```python
from statsmodels.tsa.stattools import grangercausalitytests
import pandas as pd

def discover_granger_relationships(df: pd.DataFrame, max_lag: int = 20, alpha: float = 0.05):
    """
    Discover pairwise Granger causality relationships.

    Returns: List of (cause, effect, optimal_lag, p_value) tuples
    """
    relationships = []
    columns = df.columns.tolist()

    for cause in columns:
        for effect in columns:
            if cause == effect:
                continue

            data = df[[effect, cause]].dropna()

            try:
                results = grangercausalitytests(data, maxlag=max_lag, verbose=False)

                # Find best lag (minimum p-value)
                best_lag = min(results.keys(),
                              key=lambda k: results[k][0]['ssr_ftest'][1])
                p_value = results[best_lag][0]['ssr_ftest'][1]

                if p_value < alpha:
                    relationships.append({
                        'cause': cause,
                        'effect': effect,
                        'optimal_lag': best_lag,
                        'p_value': p_value,
                        'f_statistic': results[best_lag][0]['ssr_ftest'][0]
                    })
            except Exception as e:
                continue

    return sorted(relationships, key=lambda x: x['p_value'])
```

**Limitations:**
- Assumes linear relationships
- Sensitive to lag selection
- Requires stationarity
- Correlation != Causation (despite name)

**Best For:** Detecting lead-lag relationships between financial time series

---

### 1.2 Transfer Entropy (Information Flow)

**Description:** Measures the amount of information transferred from one time series to another. Non-parametric extension of Granger causality that captures non-linear relationships.

**Mathematical Foundation:**
```
Transfer Entropy from X to Y:
T_{X->Y} = sum_{y_{t+1}, y_t, x_t} p(y_{t+1}, y_t, x_t) *
           log[ p(y_{t+1} | y_t, x_t) / p(y_{t+1} | y_t) ]

Interpretation:
- T > 0: X provides information about Y's future beyond Y's own past
- Higher T = stronger information flow
- Asymmetric: T_{X->Y} != T_{Y->X}
```

**Implementation (Python):**
```python
import numpy as np
from sklearn.neighbors import KDTree

def transfer_entropy(x: np.ndarray, y: np.ndarray, k: int = 4,
                     lag: int = 1, bins: int = 10) -> float:
    """
    Compute transfer entropy from X to Y using KSG estimator.

    Args:
        x: Source time series
        y: Target time series
        k: Number of nearest neighbors for KSG estimator
        lag: Time lag for conditioning
        bins: Number of bins for discretization (alternative method)

    Returns:
        Transfer entropy in bits
    """
    n = len(y) - lag

    # Build embedded vectors
    # Y_future | Y_past, X_past
    y_future = y[lag:].reshape(-1, 1)
    y_past = y[:-lag].reshape(-1, 1)
    x_past = x[:-lag].reshape(-1, 1)

    # Concatenate for joint spaces
    joint_xyz = np.hstack([y_future, y_past, x_past])
    joint_yz = np.hstack([y_future, y_past])
    joint_xz = np.hstack([y_past, x_past])
    joint_z = y_past

    # KSG estimator using nearest neighbors
    te = (_entropy_ksg(joint_yz, k) + _entropy_ksg(joint_xz, k)
          - _entropy_ksg(joint_xyz, k) - _entropy_ksg(joint_z, k))

    return max(0, te)  # TE should be non-negative

def _entropy_ksg(data: np.ndarray, k: int) -> float:
    """KSG entropy estimator using k-nearest neighbors."""
    n, d = data.shape
    tree = KDTree(data)

    # k+1 because point itself is included
    distances, _ = tree.query(data, k=k+1)
    epsilon = distances[:, -1]

    # Digamma function approximation
    from scipy.special import digamma
    return digamma(n) - digamma(k) + d * np.mean(np.log(2 * epsilon + 1e-10))

def bidirectional_transfer_entropy(x: np.ndarray, y: np.ndarray,
                                    max_lag: int = 10) -> dict:
    """
    Compute transfer entropy in both directions across multiple lags.
    Returns net information flow direction.
    """
    results = {
        'x_to_y': {},
        'y_to_x': {},
        'net_flow': None
    }

    for lag in range(1, max_lag + 1):
        results['x_to_y'][lag] = transfer_entropy(x, y, lag=lag)
        results['y_to_x'][lag] = transfer_entropy(y, x, lag=lag)

    # Net flow (positive = X leads Y)
    avg_x_to_y = np.mean(list(results['x_to_y'].values()))
    avg_y_to_x = np.mean(list(results['y_to_x'].values()))
    results['net_flow'] = avg_x_to_y - avg_y_to_x

    return results
```

**Advantages over Granger:**
- Captures non-linear dependencies
- Model-free (no parametric assumptions)
- Provides directional information flow

**Best For:** Detecting complex information flows between markets, sentiment and prices

---

### 1.3 Dynamic Time Warping (DTW) for Lagged Correlations

**Description:** Finds optimal alignment between time series that may have variable time delays. Discovers correlations where the lag varies over time.

**Mathematical Foundation:**
```
DTW Distance:
D(i, j) = d(x_i, y_j) + min{ D(i-1, j-1),   # Match
                             D(i-1, j),      # Insertion
                             D(i, j-1) }     # Deletion

Where d(x_i, y_j) is point-wise distance (e.g., Euclidean)

Warping Path: Sequence of (i,j) pairs showing optimal alignment
```

**Implementation:**
```python
from fastdtw import fastdtw
import numpy as np
from scipy.spatial.distance import euclidean

def dtw_correlation_search(target: np.ndarray,
                           candidates: dict,
                           window_size: int = 50,
                           max_warping: int = 20) -> list:
    """
    Find time series most correlated with target using DTW.

    Args:
        target: Target time series to find correlates for
        candidates: Dict of {name: time_series}
        window_size: Rolling window for local DTW
        max_warping: Maximum warping constraint (Sakoe-Chiba band)

    Returns:
        List of (name, dtw_distance, avg_lag) sorted by distance
    """
    results = []

    # Normalize target
    target_norm = (target - np.mean(target)) / np.std(target)

    for name, series in candidates.items():
        # Normalize candidate
        series_norm = (series - np.mean(series)) / np.std(series)

        # Compute DTW with constraint
        distance, path = fastdtw(target_norm, series_norm,
                                  dist=euclidean,
                                  radius=max_warping)

        # Compute average lag from warping path
        lags = [j - i for i, j in path]
        avg_lag = np.mean(lags)

        results.append({
            'name': name,
            'dtw_distance': distance / len(target),  # Normalize by length
            'average_lag': avg_lag,
            'lag_std': np.std(lags),  # Variability of lag
            'path': path
        })

    return sorted(results, key=lambda x: x['dtw_distance'])

def sliding_dtw_correlation(x: np.ndarray, y: np.ndarray,
                            window: int = 100, step: int = 10) -> np.ndarray:
    """
    Compute rolling DTW distance to detect regime changes in correlation.

    Returns: Array of local DTW distances over time
    """
    distances = []
    timestamps = []

    for i in range(0, len(x) - window, step):
        x_window = x[i:i+window]
        y_window = y[i:i+window]

        # Normalize
        x_norm = (x_window - np.mean(x_window)) / (np.std(x_window) + 1e-10)
        y_norm = (y_window - np.mean(y_window)) / (np.std(y_window) + 1e-10)

        dist, _ = fastdtw(x_norm, y_norm, dist=euclidean)
        distances.append(dist / window)
        timestamps.append(i + window // 2)

    return np.array(timestamps), np.array(distances)
```

**Best For:**
- Finding correlations with variable lead times
- Detecting similar patterns at different speeds
- Cross-asset contagion analysis

---

### 1.4 Mutual Information (Non-Linear Dependencies)

**Description:** Measures total dependency between variables, capturing both linear and non-linear relationships.

**Mathematical Foundation:**
```
Mutual Information:
I(X; Y) = sum_{x,y} p(x,y) * log[ p(x,y) / (p(x) * p(y)) ]

Properties:
- I(X; Y) >= 0 (always non-negative)
- I(X; Y) = 0 iff X and Y are independent
- I(X; Y) = I(Y; X) (symmetric)
- I(X; Y) <= min(H(X), H(Y)) (bounded by entropy)

For continuous variables, use KDE or KSG estimator
```

**Implementation:**
```python
from sklearn.feature_selection import mutual_info_regression
from scipy.stats import gaussian_kde
import numpy as np

def mutual_information_matrix(df: pd.DataFrame,
                               discrete_features: list = None) -> pd.DataFrame:
    """
    Compute pairwise mutual information matrix.
    """
    n_features = len(df.columns)
    mi_matrix = np.zeros((n_features, n_features))

    for i, col_i in enumerate(df.columns):
        for j, col_j in enumerate(df.columns):
            if i == j:
                mi_matrix[i, j] = 1.0  # Self-information normalized
            elif j > i:
                # Compute MI using sklearn's implementation
                mi = mutual_info_regression(
                    df[[col_i]].values,
                    df[col_j].values,
                    discrete_features=discrete_features,
                    n_neighbors=5
                )[0]
                mi_matrix[i, j] = mi
                mi_matrix[j, i] = mi  # Symmetric

    return pd.DataFrame(mi_matrix, index=df.columns, columns=df.columns)

def conditional_mutual_information(x: np.ndarray, y: np.ndarray,
                                    z: np.ndarray, k: int = 5) -> float:
    """
    Compute I(X; Y | Z) - mutual information between X and Y given Z.

    Uses KSG estimator for continuous variables.
    """
    # CMI = H(X|Z) + H(Y|Z) - H(X,Y|Z)
    # Using nearest neighbor estimator

    from sklearn.neighbors import NearestNeighbors
    from scipy.special import digamma

    n = len(x)
    xyz = np.column_stack([x, y, z])
    xz = np.column_stack([x, z])
    yz = np.column_stack([y, z])
    z_only = z.reshape(-1, 1) if z.ndim == 1 else z

    # Fit k-NN on joint space
    nn_xyz = NearestNeighbors(n_neighbors=k+1).fit(xyz)
    distances, _ = nn_xyz.kneighbors(xyz)
    eps = distances[:, -1]  # Distance to k-th neighbor

    # Count neighbors in marginal spaces within eps
    def count_neighbors(data, eps_vec):
        nn = NearestNeighbors(metric='chebyshev').fit(data)
        counts = np.array([
            len(nn.radius_neighbors([data[i]], radius=eps_vec[i],
                                    return_distance=False)[0]) - 1
            for i in range(len(data))
        ])
        return counts

    n_xz = count_neighbors(xz, eps)
    n_yz = count_neighbors(yz, eps)
    n_z = count_neighbors(z_only, eps)

    # CMI estimator
    cmi = digamma(k) - np.mean(digamma(n_xz + 1) + digamma(n_yz + 1) - digamma(n_z + 1))

    return max(0, cmi)
```

**Best For:**
- Detecting non-linear dependencies
- Feature selection for predictive models
- Finding hidden relationships that correlation misses

---

### 1.5 Copula Models for Tail Dependencies

**Description:** Captures dependency structure separate from marginal distributions. Critical for understanding correlations during market stress (when they typically increase).

**Mathematical Foundation:**
```
Sklar's Theorem:
For any joint distribution F(x,y) with marginals F_X(x) and F_Y(y):
F(x,y) = C(F_X(x), F_Y(y))

Where C is a copula function on [0,1]^2

Common Copulas:
- Gaussian: No tail dependence (inappropriate for market stress)
- Clayton: Lower tail dependence (good for crash correlation)
- Gumbel: Upper tail dependence (boom correlation)
- Student-t: Symmetric tail dependence
- Frank: No tail dependence but flexible

Tail Dependence Coefficient:
lambda_L = lim_{u->0} P(V <= u | U <= u)  (lower tail)
lambda_U = lim_{u->1} P(V > u | U > u)    (upper tail)
```

**Implementation:**
```python
from scipy import stats
from copulas.univariate import GaussianKDE
from copulas.bivariate import Clayton, Gumbel, Frank, GaussianCopula

def fit_copula_models(x: np.ndarray, y: np.ndarray) -> dict:
    """
    Fit multiple copula models and select best fit.
    """
    # Transform to uniform marginals using empirical CDF
    u = stats.rankdata(x) / (len(x) + 1)
    v = stats.rankdata(y) / (len(y) + 1)

    copulas = {
        'gaussian': GaussianCopula(),
        'clayton': Clayton(),
        'gumbel': Gumbel(),
        'frank': Frank()
    }

    results = {}
    for name, copula in copulas.items():
        try:
            copula.fit(np.column_stack([u, v]))

            # Compute AIC/BIC for model selection
            log_likelihood = np.sum(np.log(copula.pdf(np.column_stack([u, v])) + 1e-10))
            n_params = 1  # Most copulas have 1 parameter
            aic = 2 * n_params - 2 * log_likelihood

            results[name] = {
                'copula': copula,
                'log_likelihood': log_likelihood,
                'aic': aic,
                'parameter': copula.theta if hasattr(copula, 'theta') else None
            }
        except Exception as e:
            results[name] = {'error': str(e)}

    return results

def tail_dependence_analysis(x: np.ndarray, y: np.ndarray,
                              quantile: float = 0.05) -> dict:
    """
    Empirical tail dependence analysis.
    """
    n = len(x)
    u = stats.rankdata(x) / (n + 1)
    v = stats.rankdata(y) / (n + 1)

    # Lower tail: P(V <= q | U <= q)
    lower_mask = u <= quantile
    if lower_mask.sum() > 0:
        lower_tail = (v[lower_mask] <= quantile).mean()
    else:
        lower_tail = np.nan

    # Upper tail: P(V > 1-q | U > 1-q)
    upper_mask = u > (1 - quantile)
    if upper_mask.sum() > 0:
        upper_tail = (v[upper_mask] > (1 - quantile)).mean()
    else:
        upper_tail = np.nan

    # Compare to independence baseline
    return {
        'lower_tail_dependence': lower_tail,
        'upper_tail_dependence': upper_tail,
        'independence_baseline': quantile,  # What we'd expect if independent
        'lower_excess': lower_tail - quantile if not np.isnan(lower_tail) else np.nan,
        'upper_excess': upper_tail - quantile if not np.isnan(upper_tail) else np.nan
    }

def rolling_tail_dependence(x: np.ndarray, y: np.ndarray,
                            window: int = 252, quantile: float = 0.1) -> pd.DataFrame:
    """
    Track how tail dependence changes over time (regime detection).
    """
    results = []

    for i in range(window, len(x)):
        x_window = x[i-window:i]
        y_window = y[i-window:i]

        tail_dep = tail_dependence_analysis(x_window, y_window, quantile)
        tail_dep['timestamp'] = i
        results.append(tail_dep)

    return pd.DataFrame(results)
```

**Best For:**
- Understanding crisis correlations
- Risk management and hedging decisions
- Detecting correlation regime changes

---

## 2. Machine Learning for Correlation Discovery

### 2.1 Feature Importance from Ensemble Models

**Description:** Use tree-based models to discover which features (including alternative data) have predictive power.

**Implementation:**
```python
from sklearn.ensemble import RandomForestRegressor, GradientBoostingRegressor
from sklearn.inspection import permutation_importance
import shap

def ensemble_feature_discovery(X: pd.DataFrame, y: pd.Series,
                                n_estimators: int = 100) -> dict:
    """
    Discover predictive features using ensemble methods.
    """
    # Train multiple models
    models = {
        'random_forest': RandomForestRegressor(n_estimators=n_estimators, random_state=42),
        'gradient_boosting': GradientBoostingRegressor(n_estimators=n_estimators, random_state=42)
    }

    results = {}

    for name, model in models.items():
        model.fit(X, y)

        # Built-in importance (MDI for RF, gain for GB)
        builtin_importance = pd.Series(
            model.feature_importances_,
            index=X.columns
        ).sort_values(ascending=False)

        # Permutation importance (more reliable)
        perm_importance = permutation_importance(
            model, X, y, n_repeats=10, random_state=42
        )
        perm_importance_df = pd.Series(
            perm_importance.importances_mean,
            index=X.columns
        ).sort_values(ascending=False)

        results[name] = {
            'builtin_importance': builtin_importance,
            'permutation_importance': perm_importance_df,
            'model': model
        }

    # Consensus ranking
    consensus = _consensus_ranking([
        results['random_forest']['permutation_importance'],
        results['gradient_boosting']['permutation_importance']
    ])

    results['consensus'] = consensus
    return results

def _consensus_ranking(importance_series: list) -> pd.Series:
    """Borda count consensus of multiple importance rankings."""
    all_features = importance_series[0].index.tolist()
    borda_scores = {f: 0 for f in all_features}

    for imp in importance_series:
        for rank, feature in enumerate(imp.index):
            borda_scores[feature] += len(all_features) - rank

    return pd.Series(borda_scores).sort_values(ascending=False)
```

---

### 2.2 SHAP Values for Interpretability

**Description:** SHAP (SHapley Additive exPlanations) provides consistent, locally accurate feature attributions based on game theory.

**Implementation:**
```python
import shap

def shap_correlation_analysis(model, X: pd.DataFrame,
                               sample_size: int = 1000) -> dict:
    """
    Use SHAP to understand feature contributions and interactions.
    """
    # Sample for efficiency
    if len(X) > sample_size:
        X_sample = X.sample(sample_size, random_state=42)
    else:
        X_sample = X

    # Create explainer
    explainer = shap.TreeExplainer(model)
    shap_values = explainer.shap_values(X_sample)

    # Global importance (mean |SHAP|)
    global_importance = pd.Series(
        np.abs(shap_values).mean(axis=0),
        index=X.columns
    ).sort_values(ascending=False)

    # Feature interactions
    interaction_values = explainer.shap_interaction_values(X_sample)

    # Find strongest interactions
    n_features = len(X.columns)
    interactions = []
    for i in range(n_features):
        for j in range(i+1, n_features):
            interaction_strength = np.abs(interaction_values[:, i, j]).mean()
            interactions.append({
                'feature_1': X.columns[i],
                'feature_2': X.columns[j],
                'interaction_strength': interaction_strength
            })

    interactions_df = pd.DataFrame(interactions).sort_values(
        'interaction_strength', ascending=False
    )

    return {
        'global_importance': global_importance,
        'shap_values': shap_values,
        'interactions': interactions_df,
        'explainer': explainer
    }

def shap_time_varying_importance(model, X: pd.DataFrame,
                                  window: int = 252) -> pd.DataFrame:
    """
    Track how feature importance changes over time.
    """
    explainer = shap.TreeExplainer(model)

    results = []
    for i in range(window, len(X), window // 4):  # 25% overlap
        X_window = X.iloc[i-window:i]
        shap_values = explainer.shap_values(X_window)

        importance = pd.Series(
            np.abs(shap_values).mean(axis=0),
            index=X.columns
        )
        importance['timestamp'] = X.index[i]
        results.append(importance)

    return pd.DataFrame(results).set_index('timestamp')
```

---

### 2.3 Autoencoders for Latent Factor Discovery

**Description:** Neural networks that compress data into a lower-dimensional representation, potentially discovering hidden factors.

**Implementation:**
```python
import torch
import torch.nn as nn

class LatentFactorAutoencoder(nn.Module):
    """
    Autoencoder for discovering latent factors in financial data.
    """
    def __init__(self, input_dim: int, latent_dim: int = 5,
                 hidden_dims: list = [64, 32]):
        super().__init__()

        # Encoder
        encoder_layers = []
        prev_dim = input_dim
        for hidden_dim in hidden_dims:
            encoder_layers.extend([
                nn.Linear(prev_dim, hidden_dim),
                nn.BatchNorm1d(hidden_dim),
                nn.ReLU(),
                nn.Dropout(0.1)
            ])
            prev_dim = hidden_dim
        encoder_layers.append(nn.Linear(prev_dim, latent_dim))
        self.encoder = nn.Sequential(*encoder_layers)

        # Decoder
        decoder_layers = []
        prev_dim = latent_dim
        for hidden_dim in reversed(hidden_dims):
            decoder_layers.extend([
                nn.Linear(prev_dim, hidden_dim),
                nn.BatchNorm1d(hidden_dim),
                nn.ReLU(),
                nn.Dropout(0.1)
            ])
            prev_dim = hidden_dim
        decoder_layers.append(nn.Linear(prev_dim, input_dim))
        self.decoder = nn.Sequential(*decoder_layers)

    def encode(self, x):
        return self.encoder(x)

    def decode(self, z):
        return self.decoder(z)

    def forward(self, x):
        z = self.encode(x)
        return self.decode(z), z

def train_latent_factor_model(X: np.ndarray, latent_dim: int = 5,
                               epochs: int = 100, lr: float = 0.001) -> tuple:
    """
    Train autoencoder and extract latent factors.
    """
    model = LatentFactorAutoencoder(X.shape[1], latent_dim)
    optimizer = torch.optim.Adam(model.parameters(), lr=lr)
    criterion = nn.MSELoss()

    X_tensor = torch.FloatTensor(X)
    dataset = torch.utils.data.TensorDataset(X_tensor)
    loader = torch.utils.data.DataLoader(dataset, batch_size=64, shuffle=True)

    losses = []
    for epoch in range(epochs):
        epoch_loss = 0
        for batch in loader:
            x = batch[0]
            optimizer.zero_grad()
            x_recon, z = model(x)
            loss = criterion(x_recon, x)
            loss.backward()
            optimizer.step()
            epoch_loss += loss.item()
        losses.append(epoch_loss / len(loader))

    # Extract latent factors for all data
    model.eval()
    with torch.no_grad():
        _, latent_factors = model(X_tensor)

    return model, latent_factors.numpy(), losses

def analyze_latent_factors(latent_factors: np.ndarray,
                           original_features: pd.DataFrame) -> dict:
    """
    Understand what the latent factors represent.
    """
    # Correlation with original features
    factor_df = pd.DataFrame(
        latent_factors,
        columns=[f'Factor_{i}' for i in range(latent_factors.shape[1])]
    )

    correlations = factor_df.corrwith(original_features)

    # Cluster original features by factor loadings
    from sklearn.cluster import KMeans
    encoder_weights = model.encoder[0].weight.detach().numpy()  # First layer
    kmeans = KMeans(n_clusters=latent_factors.shape[1], random_state=42)
    feature_clusters = kmeans.fit_predict(encoder_weights.T)

    return {
        'factor_correlations': correlations,
        'feature_clusters': dict(zip(original_features.columns, feature_clusters)),
        'latent_factors': factor_df
    }
```

---

### 2.4 Attention Mechanisms (What Does the Model Attend To?)

**Description:** Transformer attention weights reveal which input features/time steps the model considers important.

**Implementation:**
```python
import torch
import torch.nn as nn

class AttentionCorrelationDiscovery(nn.Module):
    """
    Temporal attention model for discovering predictive time lags.
    """
    def __init__(self, n_features: int, seq_len: int, d_model: int = 64,
                 n_heads: int = 4):
        super().__init__()

        self.embedding = nn.Linear(n_features, d_model)
        self.pos_encoding = nn.Parameter(torch.randn(1, seq_len, d_model) * 0.1)

        self.attention = nn.MultiheadAttention(d_model, n_heads, batch_first=True)
        self.fc = nn.Linear(d_model, 1)

    def forward(self, x, return_attention=False):
        # x: (batch, seq_len, n_features)
        embedded = self.embedding(x) + self.pos_encoding

        # Self-attention
        attn_output, attn_weights = self.attention(
            embedded, embedded, embedded
        )

        # Predict from last timestep
        out = self.fc(attn_output[:, -1, :])

        if return_attention:
            return out, attn_weights
        return out

def extract_temporal_attention_patterns(model, X: torch.Tensor) -> dict:
    """
    Extract attention patterns to understand temporal dependencies.
    """
    model.eval()
    with torch.no_grad():
        _, attn_weights = model(X, return_attention=True)

    # attn_weights: (batch, n_heads, seq_len, seq_len)
    # Focus on attention TO the last timestep (prediction target)
    last_step_attention = attn_weights[:, :, -1, :].mean(dim=1)  # Average heads

    # Average across batch
    avg_attention = last_step_attention.mean(dim=0).numpy()

    return {
        'temporal_attention': avg_attention,
        'most_attended_lags': np.argsort(avg_attention)[::-1][:10],
        'attention_concentration': (avg_attention ** 2).sum()  # Gini-like measure
    }

class CrossAssetAttention(nn.Module):
    """
    Cross-attention to discover which assets inform others.
    """
    def __init__(self, n_assets: int, seq_len: int, d_model: int = 64):
        super().__init__()

        self.asset_embedding = nn.Embedding(n_assets, d_model)
        self.time_embedding = nn.Linear(1, d_model)
        self.cross_attention = nn.MultiheadAttention(d_model, n_heads=4, batch_first=True)

    def forward(self, x, target_asset_idx):
        # x: (batch, n_assets, seq_len)
        batch_size, n_assets, seq_len = x.shape

        # Embed each asset's time series
        asset_ids = torch.arange(n_assets).unsqueeze(0).expand(batch_size, -1)
        asset_emb = self.asset_embedding(asset_ids)  # (batch, n_assets, d_model)

        # Target asset query
        target_query = asset_emb[:, target_asset_idx:target_asset_idx+1, :]

        # Cross-attention: target queries other assets
        _, cross_attn = self.cross_attention(target_query, asset_emb, asset_emb)

        return cross_attn  # Reveals which assets inform target
```

---

### 2.5 Causal Discovery Algorithms

**Description:** Algorithms that attempt to infer causal structure from observational data.

#### PC Algorithm (Constraint-Based)
```python
from causallearn.search.ConstraintBased.PC import pc
from causallearn.utils.cit import fisherz

def pc_causal_discovery(df: pd.DataFrame, alpha: float = 0.05) -> dict:
    """
    Use PC algorithm to discover causal graph structure.
    """
    # Prepare data
    data = df.values

    # Run PC algorithm
    cg = pc(data, alpha=alpha, indep_test=fisherz)

    # Extract edges
    edges = []
    adj_matrix = cg.G.graph

    for i in range(len(df.columns)):
        for j in range(i+1, len(df.columns)):
            if adj_matrix[i, j] == 1 and adj_matrix[j, i] == 1:
                edges.append({
                    'from': df.columns[i],
                    'to': df.columns[j],
                    'type': 'undirected'
                })
            elif adj_matrix[i, j] == 1:
                edges.append({
                    'from': df.columns[i],
                    'to': df.columns[j],
                    'type': 'directed'
                })
            elif adj_matrix[j, i] == 1:
                edges.append({
                    'from': df.columns[j],
                    'to': df.columns[i],
                    'type': 'directed'
                })

    return {
        'edges': edges,
        'adjacency_matrix': adj_matrix,
        'column_names': df.columns.tolist()
    }
```

#### NOTEARS (Score-Based with Acyclicity Constraint)
```python
from notears import notears_linear

def notears_causal_discovery(df: pd.DataFrame,
                             lambda1: float = 0.1) -> dict:
    """
    Use NOTEARS for differentiable causal discovery.

    NOTEARS formulation:
    min ||X - XW||^2 + lambda * ||W||_1
    s.t. trace(exp(W o W)) - d = 0  (acyclicity constraint)
    """
    data = df.values

    # Run NOTEARS
    W_est = notears_linear(data, lambda1=lambda1, max_iter=100)

    # Threshold small weights
    W_thresholded = np.where(np.abs(W_est) > 0.1, W_est, 0)

    # Extract causal effects
    effects = []
    for i in range(len(df.columns)):
        for j in range(len(df.columns)):
            if W_thresholded[i, j] != 0:
                effects.append({
                    'cause': df.columns[i],
                    'effect': df.columns[j],
                    'strength': W_thresholded[i, j]
                })

    return {
        'weight_matrix': W_thresholded,
        'causal_effects': sorted(effects, key=lambda x: abs(x['strength']), reverse=True),
        'columns': df.columns.tolist()
    }
```

---

## 3. Handling Non-Stationarity

### 3.1 Rolling Correlation Analysis

**Implementation:**
```python
def rolling_correlation_heatmap(df: pd.DataFrame,
                                 window: int = 252,
                                 step: int = 21) -> dict:
    """
    Track how correlations evolve over time.
    """
    timestamps = []
    correlation_matrices = []

    for i in range(window, len(df), step):
        window_data = df.iloc[i-window:i]
        corr = window_data.corr()
        timestamps.append(df.index[i])
        correlation_matrices.append(corr.values)

    # Convert to 3D array
    corr_tensor = np.stack(correlation_matrices)

    # Identify most volatile correlations
    corr_volatility = np.std(corr_tensor, axis=0)

    return {
        'timestamps': timestamps,
        'correlations': corr_tensor,
        'volatility': pd.DataFrame(corr_volatility,
                                    index=df.columns,
                                    columns=df.columns),
        'columns': df.columns.tolist()
    }

def correlation_regime_detection(corr_time_series: np.ndarray,
                                  n_regimes: int = 3) -> dict:
    """
    Detect correlation regimes using HMM.
    """
    from hmmlearn import hmm

    # Flatten correlation matrix to vector (upper triangle)
    n_assets = corr_time_series.shape[1]
    triu_idx = np.triu_indices(n_assets, k=1)

    corr_vectors = np.array([
        corr[triu_idx] for corr in corr_time_series
    ])

    # Fit HMM
    model = hmm.GaussianHMM(n_components=n_regimes, covariance_type='full',
                            n_iter=100, random_state=42)
    model.fit(corr_vectors)

    # Predict regimes
    regimes = model.predict(corr_vectors)

    # Characterize each regime
    regime_stats = {}
    for r in range(n_regimes):
        mask = regimes == r
        regime_corrs = corr_vectors[mask]
        regime_stats[r] = {
            'mean_correlation': regime_corrs.mean(),
            'std_correlation': regime_corrs.std(),
            'frequency': mask.mean(),
            'typical_matrix': corr_time_series[mask].mean(axis=0)
        }

    return {
        'regimes': regimes,
        'regime_stats': regime_stats,
        'transition_matrix': model.transmat_,
        'model': model
    }
```

### 3.2 Regime-Conditional Correlations

```python
def regime_conditional_correlation(df: pd.DataFrame,
                                    regime_indicator: pd.Series) -> dict:
    """
    Compute correlations conditional on market regime.

    Args:
        df: Asset returns
        regime_indicator: Series with regime labels (e.g., 'bull', 'bear', 'neutral')
    """
    results = {}

    for regime in regime_indicator.unique():
        mask = regime_indicator == regime
        regime_data = df.loc[mask]

        results[regime] = {
            'correlation': regime_data.corr(),
            'n_observations': mask.sum(),
            'proportion': mask.mean()
        }

    # Compare correlations across regimes
    base_regime = list(results.keys())[0]
    base_corr = results[base_regime]['correlation']

    for regime, data in results.items():
        if regime != base_regime:
            diff = data['correlation'] - base_corr
            results[regime]['correlation_difference'] = diff
            results[regime]['max_change'] = np.abs(diff.values).max()

    return results

def vix_conditional_correlation(returns_df: pd.DataFrame,
                                 vix: pd.Series,
                                 quantiles: list = [0.25, 0.75]) -> dict:
    """
    Correlations conditional on VIX level.
    """
    vix_aligned = vix.reindex(returns_df.index)

    low_vix = vix_aligned <= vix_aligned.quantile(quantiles[0])
    high_vix = vix_aligned >= vix_aligned.quantile(quantiles[1])
    mid_vix = ~low_vix & ~high_vix

    return {
        'low_vix': {
            'correlation': returns_df.loc[low_vix].corr(),
            'vix_range': (vix_aligned[low_vix].min(), vix_aligned[low_vix].max()),
            'n_obs': low_vix.sum()
        },
        'mid_vix': {
            'correlation': returns_df.loc[mid_vix].corr(),
            'vix_range': (vix_aligned[mid_vix].min(), vix_aligned[mid_vix].max()),
            'n_obs': mid_vix.sum()
        },
        'high_vix': {
            'correlation': returns_df.loc[high_vix].corr(),
            'vix_range': (vix_aligned[high_vix].min(), vix_aligned[high_vix].max()),
            'n_obs': high_vix.sum()
        }
    }
```

### 3.3 Correlation Breakdown During Crises

```python
def crisis_correlation_analysis(returns_df: pd.DataFrame,
                                 crisis_dates: list,
                                 pre_window: int = 252,
                                 crisis_window: int = 63,
                                 post_window: int = 126) -> dict:
    """
    Analyze how correlations change during crisis periods.
    """
    results = {}

    for crisis_name, crisis_date in crisis_dates:
        crisis_idx = returns_df.index.get_loc(crisis_date)

        pre_data = returns_df.iloc[crisis_idx-pre_window:crisis_idx]
        crisis_data = returns_df.iloc[crisis_idx:crisis_idx+crisis_window]
        post_data = returns_df.iloc[crisis_idx+crisis_window:crisis_idx+crisis_window+post_window]

        pre_corr = pre_data.corr()
        crisis_corr = crisis_data.corr()
        post_corr = post_data.corr()

        # Compute correlation changes
        crisis_change = crisis_corr - pre_corr
        post_change = post_corr - pre_corr

        # Average correlation (excluding diagonal)
        def avg_corr(corr_matrix):
            mask = ~np.eye(len(corr_matrix), dtype=bool)
            return corr_matrix.values[mask].mean()

        results[crisis_name] = {
            'pre_crisis_avg_corr': avg_corr(pre_corr),
            'crisis_avg_corr': avg_corr(crisis_corr),
            'post_crisis_avg_corr': avg_corr(post_corr),
            'correlation_spike': avg_corr(crisis_corr) - avg_corr(pre_corr),
            'recovery': avg_corr(post_corr) - avg_corr(crisis_corr),
            'pairs_most_affected': _top_correlation_changes(crisis_change, returns_df.columns, top_n=5)
        }

    return results

def _top_correlation_changes(change_matrix: pd.DataFrame,
                              columns: list, top_n: int = 5) -> list:
    """Find pairs with largest correlation changes."""
    changes = []
    for i, col_i in enumerate(columns):
        for j, col_j in enumerate(columns):
            if j > i:
                changes.append({
                    'pair': (col_i, col_j),
                    'change': change_matrix.iloc[i, j]
                })

    return sorted(changes, key=lambda x: abs(x['change']), reverse=True)[:top_n]
```

### 3.4 Adaptive Correlation Estimation (DCC-GARCH)

```python
from arch import arch_model

def dcc_garch_correlation(returns_df: pd.DataFrame) -> dict:
    """
    Dynamic Conditional Correlation using GARCH models.

    DCC Model:
    1. Fit univariate GARCH for each asset
    2. Estimate time-varying correlations using standardized residuals
    """
    n_assets = len(returns_df.columns)

    # Step 1: Fit univariate GARCH models
    standardized_residuals = pd.DataFrame(index=returns_df.index)
    conditional_volatilities = pd.DataFrame(index=returns_df.index)

    for col in returns_df.columns:
        model = arch_model(returns_df[col].dropna(), vol='Garch', p=1, q=1)
        result = model.fit(disp='off')

        std_resid = result.std_resid
        cond_vol = result.conditional_volatility

        standardized_residuals[col] = std_resid
        conditional_volatilities[col] = cond_vol

    # Step 2: Compute time-varying correlation
    # Simplified: EWMA correlation of standardized residuals
    lambda_decay = 0.94

    correlations = []
    Q = standardized_residuals.iloc[:100].cov().values  # Initial estimate
    Q_bar = Q.copy()

    for t in range(100, len(standardized_residuals)):
        eps = standardized_residuals.iloc[t].values.reshape(-1, 1)

        # DCC dynamics
        Q = (1 - lambda_decay) * Q_bar + lambda_decay * (eps @ eps.T)

        # Normalize to correlation
        D = np.sqrt(np.diag(Q))
        R = Q / np.outer(D, D)

        correlations.append(R)

    return {
        'dynamic_correlations': correlations,
        'timestamps': returns_df.index[100:],
        'conditional_volatilities': conditional_volatilities,
        'columns': returns_df.columns.tolist()
    }
```

---

## 4. Famous Non-Obvious Correlations

### 4.1 Baltic Dry Index and Global Growth

**Relationship:** The BDI measures shipping costs for dry bulk commodities. It leads global industrial production by 2-4 months.

**Economic Rationale:**
- Shipping demand reflects actual trade flows (not financial speculation)
- Companies order materials before production increases
- Supply is relatively inelastic in short-term

**Evidence:**
```python
# Typical lead-lag relationship
correlations = {
    'contemporaneous': 0.35,
    '1_month_lead': 0.42,
    '2_month_lead': 0.48,
    '3_month_lead': 0.45,
    '4_month_lead': 0.38
}
```

**Caveats:**
- Supply shocks (new ship deliveries) can distort signal
- China-specific demand now dominates
- Less useful post-2008 due to financialization

---

### 4.2 Copper/Gold Ratio and Interest Rates

**Relationship:** Cu/Au ratio correlates with 10-year Treasury yields (r > 0.7 historically).

**Economic Rationale:**
- Copper = industrial metal (economic growth proxy)
- Gold = safe haven (inflation/uncertainty hedge)
- Rising Cu/Au = growth optimism = higher rates
- Falling Cu/Au = risk aversion = flight to safety = lower rates

**Implementation:**
```python
def copper_gold_rates_analysis(copper: pd.Series, gold: pd.Series,
                                rates: pd.Series, window: int = 252) -> dict:
    """
    Analyze Cu/Au ratio as rates predictor.
    """
    ratio = copper / gold
    ratio_change = ratio.pct_change(21)  # Monthly change
    rates_change = rates.diff(21)  # Monthly change in bps

    # Lead-lag analysis
    lead_lags = {}
    for lag in range(-3, 4):  # -3 to +3 months
        shifted_ratio = ratio_change.shift(lag * 21)
        corr = shifted_ratio.corr(rates_change)
        lead_lags[lag] = corr

    # Rolling correlation stability
    rolling_corr = ratio_change.rolling(window).corr(rates_change)

    return {
        'lead_lag_correlations': lead_lags,
        'rolling_correlation': rolling_corr,
        'optimal_lag': max(lead_lags, key=lead_lags.get),
        'current_signal': 'higher_rates' if ratio_change.iloc[-1] > 0 else 'lower_rates'
    }
```

---

### 4.3 High Yield Spreads and Equity

**Relationship:** HY spreads lead equity by 1-3 months (inverse relationship).

**Economic Rationale:**
- HY spreads reflect credit risk assessment by bond market
- Bond market generally more informed than equity
- Credit deterioration precedes equity selloffs
- Credit improvement signals risk-on

**Evidence:**
```python
# Typical correlations
correlations = {
    'HY_spread_vs_SPX_1m_forward': -0.45,
    'HY_spread_vs_SPX_2m_forward': -0.52,
    'HY_spread_change_vs_SPX': -0.65
}
```

---

### 4.4 Dollar and Emerging Markets

**Relationship:** Strong dollar = EM weakness (correlation ~ -0.6 to -0.8).

**Economic Rationale:**
- EM debt often denominated in USD
- Strong dollar increases debt burden
- Capital flows toward USD in risk-off
- Commodity prices (EM exports) inversely correlated with USD

**Implementation:**
```python
def dollar_em_analysis(dxy: pd.Series, em_index: pd.Series) -> dict:
    """
    Analyze USD/EM relationship across regimes.
    """
    # Contemporaneous
    contemp_corr = dxy.pct_change(21).corr(em_index.pct_change(21))

    # Regime-dependent (using DXY momentum)
    dxy_up = dxy.pct_change(63) > 0
    dxy_down = ~dxy_up

    corr_dxy_up = dxy.pct_change(21)[dxy_up].corr(em_index.pct_change(21)[dxy_up])
    corr_dxy_down = dxy.pct_change(21)[dxy_down].corr(em_index.pct_change(21)[dxy_down])

    return {
        'overall_correlation': contemp_corr,
        'correlation_dollar_strengthening': corr_dxy_up,
        'correlation_dollar_weakening': corr_dxy_down,
        'asymmetry': abs(corr_dxy_up) - abs(corr_dxy_down)
    }
```

---

### 4.5 VIX Term Structure and Returns

**Relationship:** VIX contango (front < back) = bullish. Backwardation = bearish.

**Economic Rationale:**
- Normal market: VIX term structure in contango (insurance premium for longer protection)
- Stressed market: Backwardation (immediate fear > future fear)
- Contango provides roll yield for VIX shorts

**Implementation:**
```python
def vix_term_structure_signal(vix: pd.Series, vix3m: pd.Series,
                               spx_returns: pd.Series) -> dict:
    """
    Analyze VIX term structure as return predictor.
    """
    # Term structure (positive = contango)
    term_structure = (vix3m - vix) / vix

    # Forward returns by term structure quintile
    ts_quintiles = pd.qcut(term_structure, 5, labels=False)

    forward_returns = {}
    for q in range(5):
        mask = ts_quintiles == q
        fwd_ret = spx_returns.shift(-21)[mask].mean() * 12  # Annualized
        forward_returns[f'quintile_{q+1}'] = fwd_ret

    # Signal
    current_ts = term_structure.iloc[-1]

    return {
        'current_term_structure': current_ts,
        'is_contango': current_ts > 0,
        'forward_returns_by_quintile': forward_returns,
        'signal': 'bullish' if current_ts > term_structure.quantile(0.6) else
                  'bearish' if current_ts < term_structure.quantile(0.4) else 'neutral'
    }
```

---

### 4.6 Additional Non-Obvious Correlations Catalog

| Signal | Predicts | Lead Time | Economic Logic |
|--------|----------|-----------|----------------|
| **Lumber/Gold Ratio** | Housing starts | 2-3 months | Construction demand |
| **Soybean/Corn Ratio** | Farmer planting decisions | 6 months | Profit maximization |
| **TED Spread** | Credit stress | Real-time | Interbank trust |
| **LIBOR-OIS Spread** | Banking stress | Real-time | Counterparty risk |
| **Yield Curve Slope** | Recession | 12-18 months | Monetary policy expectations |
| **Corporate Bond Issuance** | Market tops | 1-2 quarters | Cheap financing exhaustion |
| **IPO Volume** | Equity peaks | 1-2 quarters | Sentiment indicator |
| **Margin Debt** | Market reversals | 2-4 months | Leverage extremes |
| **Put/Call Ratio** | Short-term reversals | Days-weeks | Sentiment extremes |
| **Breadth (A/D Line)** | Index divergence | Weeks-months | Participation quality |
| **Senior Loan Officer Survey** | Credit conditions | 1-2 quarters | Lending standards |
| **PMI New Orders-Inventories** | Manufacturing cycle | 1-3 months | Demand-supply gap |

---

## 5. Avoiding Spurious Correlations

### 5.1 Multiple Testing Corrections

**Problem:** Testing many correlations increases false positive rate.

**Solutions:**
```python
from scipy import stats
from statsmodels.stats.multitest import multipletests

def corrected_correlation_test(df: pd.DataFrame, alpha: float = 0.05) -> dict:
    """
    Test all pairwise correlations with multiple testing correction.
    """
    n_cols = len(df.columns)
    n_tests = n_cols * (n_cols - 1) // 2

    p_values = []
    correlations = []
    pairs = []

    for i in range(n_cols):
        for j in range(i+1, n_cols):
            corr, p_value = stats.pearsonr(
                df.iloc[:, i].dropna(),
                df.iloc[:, j].dropna()
            )
            p_values.append(p_value)
            correlations.append(corr)
            pairs.append((df.columns[i], df.columns[j]))

    # Apply corrections
    corrections = {
        'bonferroni': multipletests(p_values, alpha=alpha, method='bonferroni'),
        'fdr_bh': multipletests(p_values, alpha=alpha, method='fdr_bh'),  # Benjamini-Hochberg
        'fdr_by': multipletests(p_values, alpha=alpha, method='fdr_by')   # Benjamini-Yekutieli
    }

    results = []
    for i, (pair, corr, p) in enumerate(zip(pairs, correlations, p_values)):
        results.append({
            'pair': pair,
            'correlation': corr,
            'p_value': p,
            'significant_bonferroni': corrections['bonferroni'][0][i],
            'significant_fdr_bh': corrections['fdr_bh'][0][i],
            'significant_fdr_by': corrections['fdr_by'][0][i]
        })

    return {
        'results': sorted(results, key=lambda x: x['p_value']),
        'n_tests': n_tests,
        'bonferroni_threshold': alpha / n_tests,
        'n_significant_bonferroni': sum(corrections['bonferroni'][0]),
        'n_significant_fdr': sum(corrections['fdr_bh'][0])
    }
```

---

### 5.2 Out-of-Sample Validation

```python
def walk_forward_correlation_validation(df: pd.DataFrame,
                                         pair: tuple,
                                         train_window: int = 252,
                                         test_window: int = 63) -> dict:
    """
    Validate correlation stability with walk-forward testing.
    """
    col_x, col_y = pair
    correlations = []

    for i in range(train_window, len(df) - test_window, test_window):
        # Train period
        train_data = df.iloc[i-train_window:i]
        train_corr = train_data[col_x].corr(train_data[col_y])

        # Test period
        test_data = df.iloc[i:i+test_window]
        test_corr = test_data[col_x].corr(test_data[col_y])

        correlations.append({
            'train_end': df.index[i],
            'train_corr': train_corr,
            'test_corr': test_corr,
            'correlation_change': test_corr - train_corr
        })

    # Statistics
    train_corrs = [c['train_corr'] for c in correlations]
    test_corrs = [c['test_corr'] for c in correlations]

    return {
        'walk_forward_results': correlations,
        'train_mean_corr': np.mean(train_corrs),
        'test_mean_corr': np.mean(test_corrs),
        'degradation': np.mean(train_corrs) - np.mean(test_corrs),
        'stability': 1 - np.std([c['correlation_change'] for c in correlations]),
        'sign_consistency': np.mean([np.sign(c['train_corr']) == np.sign(c['test_corr'])
                                     for c in correlations])
    }
```

---

### 5.3 Economic Intuition Filters

```python
def economic_rationality_filter(discovered_correlations: list,
                                 known_relationships: dict) -> list:
    """
    Filter correlations based on economic plausibility.

    Args:
        discovered_correlations: List of {pair, correlation, p_value}
        known_relationships: Dict of {(asset1, asset2): {'expected_sign': +1/-1, 'rationale': str}}
    """
    filtered = []

    for corr in discovered_correlations:
        pair = corr['pair']
        normalized_pair = tuple(sorted(pair))
        reversed_pair = tuple(reversed(normalized_pair))

        # Check if relationship has known economic logic
        if normalized_pair in known_relationships:
            expected = known_relationships[normalized_pair]
            actual_sign = np.sign(corr['correlation'])

            if actual_sign == expected['expected_sign']:
                corr['economic_rationale'] = expected['rationale']
                corr['rationale_match'] = True
                filtered.append(corr)
            else:
                corr['warning'] = f"Sign mismatch: expected {expected['expected_sign']}, got {actual_sign}"
                corr['rationale_match'] = False
                # Still include but flag
                filtered.append(corr)
        else:
            # Unknown relationship - requires manual review
            corr['status'] = 'needs_rationale'
            corr['rationale_match'] = None
            filtered.append(corr)

    return filtered

# Example known relationships
KNOWN_RELATIONSHIPS = {
    ('USD', 'EM_Equity'): {'expected_sign': -1, 'rationale': 'EM debt burden increases with strong USD'},
    ('VIX', 'SPX'): {'expected_sign': -1, 'rationale': 'Fear gauge inversely related to equity'},
    ('Oil', 'Airlines'): {'expected_sign': -1, 'rationale': 'Fuel costs impact margins'},
    ('Copper', 'Rates'): {'expected_sign': 1, 'rationale': 'Industrial demand = growth = higher rates'},
    ('Gold', 'Real_Rates'): {'expected_sign': -1, 'rationale': 'Opportunity cost of holding non-yielding asset'},
    ('HY_Spread', 'SPX'): {'expected_sign': -1, 'rationale': 'Credit stress precedes equity selloff'},
}
```

---

### 5.4 Correlation Stability Over Time

```python
def correlation_stability_analysis(df: pd.DataFrame, pair: tuple,
                                    windows: list = [126, 252, 504]) -> dict:
    """
    Analyze correlation stability across multiple timeframes.
    """
    col_x, col_y = pair
    stability_metrics = {}

    for window in windows:
        rolling_corr = df[col_x].rolling(window).corr(df[col_y])

        stability_metrics[f'{window}d'] = {
            'mean': rolling_corr.mean(),
            'std': rolling_corr.std(),
            'min': rolling_corr.min(),
            'max': rolling_corr.max(),
            'coefficient_of_variation': rolling_corr.std() / abs(rolling_corr.mean()),
            'pct_positive': (rolling_corr > 0).mean(),
            'pct_above_0.3': (rolling_corr.abs() > 0.3).mean()
        }

    # Structural break test (Chow test approximation)
    mid_point = len(df) // 2
    first_half_corr = df.iloc[:mid_point][col_x].corr(df.iloc[:mid_point][col_y])
    second_half_corr = df.iloc[mid_point:][col_x].corr(df.iloc[mid_point:][col_y])

    stability_metrics['structural_change'] = {
        'first_half_corr': first_half_corr,
        'second_half_corr': second_half_corr,
        'change': second_half_corr - first_half_corr,
        'stable': abs(second_half_corr - first_half_corr) < 0.2
    }

    return stability_metrics
```

---

### 5.5 Distinguishing Correlation from Causation

```python
def correlation_vs_causation_tests(x: pd.Series, y: pd.Series,
                                    confounders: pd.DataFrame = None) -> dict:
    """
    Run multiple tests to assess causal plausibility.
    """
    results = {}

    # 1. Temporal precedence (Granger)
    from statsmodels.tsa.stattools import grangercausalitytests

    for lag in [1, 5, 10, 21]:
        try:
            gc_xy = grangercausalitytests(pd.DataFrame({'y': y, 'x': x}).dropna(),
                                          maxlag=lag, verbose=False)
            gc_yx = grangercausalitytests(pd.DataFrame({'x': x, 'y': y}).dropna(),
                                          maxlag=lag, verbose=False)

            results[f'granger_{lag}d'] = {
                'x_causes_y_pvalue': gc_xy[lag][0]['ssr_ftest'][1],
                'y_causes_x_pvalue': gc_yx[lag][0]['ssr_ftest'][1]
            }
        except:
            pass

    # 2. Intervention simulation (what-if)
    # If we observe X shock, does Y follow as predicted?
    x_shocks = x.pct_change().abs() > x.pct_change().std() * 2

    post_shock_y = y.shift(-5)[x_shocks].mean()  # Y after X shock
    normal_y = y.shift(-5)[~x_shocks].mean()

    results['intervention_effect'] = {
        'post_shock_mean': post_shock_y,
        'normal_mean': normal_y,
        'difference': post_shock_y - normal_y
    }

    # 3. Confounder check
    if confounders is not None:
        # Partial correlation controlling for confounders
        from scipy import stats

        residuals_x = x - confounders.apply(lambda c: c.corr(x) * c).sum(axis=1)
        residuals_y = y - confounders.apply(lambda c: c.corr(y) * c).sum(axis=1)

        partial_corr = residuals_x.corr(residuals_y)
        raw_corr = x.corr(y)

        results['confounder_analysis'] = {
            'raw_correlation': raw_corr,
            'partial_correlation': partial_corr,
            'confounding_effect': raw_corr - partial_corr
        }

    return results
```

---

## 6. Implementation Framework for NDP

### 6.1 Correlation Discovery Pipeline

```python
class CorrelationDiscoveryPipeline:
    """
    End-to-end pipeline for discovering non-obvious correlations.
    """

    def __init__(self, config: dict):
        self.config = config
        self.results = {}

    def run(self, data: pd.DataFrame, target: str = None) -> dict:
        """
        Execute full discovery pipeline.

        Args:
            data: DataFrame with all candidate time series
            target: Optional target variable to find predictors for
        """
        # Step 1: Data preparation
        clean_data = self._prepare_data(data)

        # Step 2: Linear correlation screening
        print("Running linear correlation screening...")
        linear_results = self._linear_correlation_screen(clean_data)

        # Step 3: Non-linear dependency detection
        print("Running mutual information analysis...")
        mi_results = self._mutual_information_screen(clean_data)

        # Step 4: Lead-lag analysis
        print("Running Granger causality tests...")
        granger_results = self._granger_analysis(clean_data, target)

        # Step 5: Tail dependence
        print("Analyzing tail dependencies...")
        tail_results = self._tail_dependence_analysis(clean_data)

        # Step 6: Combine and filter
        print("Combining results and applying filters...")
        combined = self._combine_results(
            linear_results, mi_results, granger_results, tail_results
        )

        # Step 7: Validation
        print("Running out-of-sample validation...")
        validated = self._validate_correlations(combined, clean_data)

        # Step 8: Economic filter
        print("Applying economic rationality filter...")
        final = self._apply_economic_filter(validated)

        self.results = final
        return final

    def _prepare_data(self, data: pd.DataFrame) -> pd.DataFrame:
        """Standardize and clean data."""
        # Handle missing values
        data = data.ffill().dropna()

        # Standardize
        standardized = (data - data.mean()) / data.std()

        return standardized

    def _linear_correlation_screen(self, data: pd.DataFrame) -> list:
        """Initial linear correlation screening."""
        results = []
        corr_matrix = data.corr()

        for i in range(len(data.columns)):
            for j in range(i+1, len(data.columns)):
                col_i, col_j = data.columns[i], data.columns[j]
                corr = corr_matrix.iloc[i, j]

                if abs(corr) > self.config.get('min_correlation', 0.3):
                    results.append({
                        'pair': (col_i, col_j),
                        'correlation': corr,
                        'method': 'pearson'
                    })

        return results

    def _mutual_information_screen(self, data: pd.DataFrame) -> list:
        """Non-linear dependency detection."""
        mi_matrix = mutual_information_matrix(data)
        results = []

        for i in range(len(data.columns)):
            for j in range(i+1, len(data.columns)):
                col_i, col_j = data.columns[i], data.columns[j]
                mi = mi_matrix.iloc[i, j]

                if mi > self.config.get('min_mi', 0.1):
                    results.append({
                        'pair': (col_i, col_j),
                        'mutual_information': mi,
                        'method': 'mutual_information'
                    })

        return results

    def _granger_analysis(self, data: pd.DataFrame, target: str = None) -> list:
        """Lead-lag relationship discovery."""
        if target:
            columns = [c for c in data.columns if c != target]
            results = discover_granger_relationships(
                data[[target] + columns],
                max_lag=self.config.get('max_lag', 20),
                alpha=self.config.get('granger_alpha', 0.05)
            )
        else:
            results = discover_granger_relationships(
                data,
                max_lag=self.config.get('max_lag', 20),
                alpha=self.config.get('granger_alpha', 0.05)
            )

        return [{'pair': (r['cause'], r['effect']), **r, 'method': 'granger'}
                for r in results]

    def _tail_dependence_analysis(self, data: pd.DataFrame) -> list:
        """Analyze tail dependencies."""
        results = []

        for i in range(len(data.columns)):
            for j in range(i+1, len(data.columns)):
                col_i, col_j = data.columns[i], data.columns[j]

                tail_dep = tail_dependence_analysis(
                    data[col_i].values,
                    data[col_j].values,
                    quantile=self.config.get('tail_quantile', 0.05)
                )

                if (tail_dep['lower_excess'] > 0.1 or
                    tail_dep['upper_excess'] > 0.1):
                    results.append({
                        'pair': (col_i, col_j),
                        **tail_dep,
                        'method': 'tail_dependence'
                    })

        return results

    def _combine_results(self, *result_lists) -> list:
        """Combine results from multiple methods."""
        combined = {}

        for results in result_lists:
            for r in results:
                pair = tuple(sorted(r['pair']))
                if pair not in combined:
                    combined[pair] = {'pair': pair, 'methods': {}}
                combined[pair]['methods'][r['method']] = r

        # Score by number of methods that found relationship
        for pair, data in combined.items():
            data['n_methods'] = len(data['methods'])
            data['confidence_score'] = data['n_methods'] / 4  # 4 methods total

        return sorted(combined.values(), key=lambda x: x['confidence_score'], reverse=True)

    def _validate_correlations(self, correlations: list, data: pd.DataFrame) -> list:
        """Out-of-sample validation."""
        validated = []

        for corr in correlations:
            validation = walk_forward_correlation_validation(
                data,
                corr['pair'],
                train_window=self.config.get('train_window', 252),
                test_window=self.config.get('test_window', 63)
            )

            corr['validation'] = validation
            corr['stable'] = (validation['sign_consistency'] > 0.7 and
                             validation['degradation'] < 0.2)
            validated.append(corr)

        return validated

    def _apply_economic_filter(self, correlations: list) -> list:
        """Apply economic rationality filter."""
        return economic_rationality_filter(
            correlations,
            self.config.get('known_relationships', KNOWN_RELATIONSHIPS)
        )

# Configuration
DISCOVERY_CONFIG = {
    'min_correlation': 0.3,
    'min_mi': 0.1,
    'max_lag': 20,
    'granger_alpha': 0.05,
    'tail_quantile': 0.05,
    'train_window': 252,
    'test_window': 63,
    'known_relationships': KNOWN_RELATIONSHIPS
}
```

### 6.2 Database Schema for Correlation Storage

```sql
-- Store discovered correlations
CREATE TABLE discovered_correlations (
    id SERIAL PRIMARY KEY,
    pair_asset_1 TEXT NOT NULL,
    pair_asset_2 TEXT NOT NULL,
    correlation_value DOUBLE PRECISION,
    mutual_information DOUBLE PRECISION,
    granger_pvalue DOUBLE PRECISION,
    optimal_lag INTEGER,
    lower_tail_dependence DOUBLE PRECISION,
    upper_tail_dependence DOUBLE PRECISION,
    n_methods_detected INTEGER,
    confidence_score DOUBLE PRECISION,
    is_stable BOOLEAN,
    sign_consistency DOUBLE PRECISION,
    economic_rationale TEXT,
    rationale_verified BOOLEAN,
    discovery_date TIMESTAMPTZ DEFAULT NOW(),
    last_validated TIMESTAMPTZ,
    validation_status TEXT,
    UNIQUE(pair_asset_1, pair_asset_2, discovery_date)
);

-- Track correlation stability over time
CREATE TABLE correlation_history (
    id SERIAL PRIMARY KEY,
    correlation_id INTEGER REFERENCES discovered_correlations(id),
    observation_date TIMESTAMPTZ NOT NULL,
    rolling_correlation DOUBLE PRECISION,
    window_days INTEGER,
    regime TEXT,
    UNIQUE(correlation_id, observation_date, window_days)
);

-- Store validation results
CREATE TABLE correlation_validation (
    id SERIAL PRIMARY KEY,
    correlation_id INTEGER REFERENCES discovered_correlations(id),
    validation_date TIMESTAMPTZ DEFAULT NOW(),
    train_period_start DATE,
    train_period_end DATE,
    test_period_start DATE,
    test_period_end DATE,
    train_correlation DOUBLE PRECISION,
    test_correlation DOUBLE PRECISION,
    degradation DOUBLE PRECISION,
    passed BOOLEAN
);

-- Index for efficient queries
CREATE INDEX idx_correlations_confidence ON discovered_correlations(confidence_score DESC);
CREATE INDEX idx_correlations_stable ON discovered_correlations(is_stable) WHERE is_stable = TRUE;
CREATE INDEX idx_history_date ON correlation_history(observation_date);
```

### 6.3 Recommended Discovery Pipeline

```
┌─────────────────────────────────────────────────────────────────────┐
│                  CORRELATION DISCOVERY PIPELINE                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  1. DATA COLLECTION                                                  │
│     ├── Market data (prices, volumes, rates)                        │
│     ├── Alternative data (sentiment, flows, positioning)            │
│     ├── Economic indicators (macro, surveys, PMIs)                  │
│     └── Derived metrics (spreads, ratios, term structures)          │
│                                                                      │
│  2. PREPROCESSING                                                    │
│     ├── Align timestamps                                            │
│     ├── Handle missing data                                         │
│     ├── Stationarity transforms (returns, differences)              │
│     └── Outlier treatment                                           │
│                                                                      │
│  3. MULTI-METHOD SCREENING                                          │
│     ├── Linear correlation (Pearson, Spearman)                      │
│     ├── Mutual information (non-linear)                             │
│     ├── Granger causality (lead-lag)                                │
│     ├── Transfer entropy (information flow)                         │
│     ├── Copula analysis (tail dependence)                           │
│     └── DTW correlation (variable lag)                              │
│                                                                      │
│  4. MACHINE LEARNING DISCOVERY                                       │
│     ├── SHAP feature importance                                     │
│     ├── Attention weight analysis                                   │
│     ├── Autoencoder latent factors                                  │
│     └── Causal discovery (PC, NOTEARS)                              │
│                                                                      │
│  5. FILTERING & VALIDATION                                          │
│     ├── Multiple testing correction (FDR)                           │
│     ├── Walk-forward validation                                     │
│     ├── Stability analysis (rolling windows)                        │
│     ├── Regime conditioning                                         │
│     └── Economic rationality check                                  │
│                                                                      │
│  6. MONITORING & MAINTENANCE                                        │
│     ├── Real-time correlation tracking                              │
│     ├── Regime change detection                                     │
│     ├── Decay/breakdown alerts                                      │
│     └── Periodic revalidation                                       │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 7. References

### Statistical Methods
- Granger, C.W.J. (1969). "Investigating Causal Relations by Econometric Models and Cross-spectral Methods"
- Schreiber, T. (2000). "Measuring Information Transfer"
- Sklar, A. (1959). "Fonctions de repartition a n dimensions et leurs marges"

### Machine Learning
- Lundberg, S. & Lee, S.I. (2017). "A Unified Approach to Interpreting Model Predictions" (SHAP)
- Kingma, D.P. & Welling, M. (2013). "Auto-Encoding Variational Bayes"
- Vaswani, A. et al. (2017). "Attention Is All You Need"

### Causal Discovery
- Spirtes, P. et al. (2000). "Causation, Prediction, and Search"
- Zheng, X. et al. (2018). "DAGs with NO TEARS: Continuous Optimization for Structure Learning"
- Pearl, J. (2009). "Causality: Models, Reasoning and Inference"

### Financial Applications
- Engle, R. (2002). "Dynamic Conditional Correlation: A Simple Class of Multivariate GARCH Models"
- Lo, A.W. (2004). "The Adaptive Markets Hypothesis"
- Harvey, C. et al. (2016). "...and the Cross-Section of Expected Returns"

### NDP Project Context
- [Time-Series Features Research](/workspaces/neural-data-platform/product/research/gold/feature-engineering/TIME-SERIES-FEATURES.md)
- [Unsupervised Learning for Edge](/workspaces/neural-data-platform/product/research/gold/unsupervised-learning/EDGE-UNSUPERVISED.md)
- [Self-Learning Systems](/workspaces/neural-data-platform/product/research/gold/self-learning/ADAPTIVE-SYSTEMS.md)

---

## 8. Conclusion

Discovering non-obvious correlations requires a multi-method approach that combines:

1. **Statistical rigor** - Multiple testing correction, out-of-sample validation
2. **Economic intuition** - Rationale for why the relationship should exist
3. **Temporal analysis** - Lead-lag relationships and regime conditioning
4. **Modern ML** - SHAP, attention, autoencoders for pattern discovery
5. **Continuous monitoring** - Correlations change; what worked may stop working

**Key Principles:**
- No correlation without economic rationale
- Always validate out-of-sample
- Monitor for regime changes and decay
- Prefer stable relationships over strong but unstable ones
- Multiple methods agreeing increases confidence

**For NDP Implementation:**
- Start with Granger causality and mutual information for screening
- Use SHAP for ML model interpretability
- Store and track discovered correlations over time
- Implement alerts for correlation breakdown
- Maintain a catalog of known relationships with rationales

---

**Document Version:** 1.0
**Author:** Research Agent
**Status:** Complete
**Last Updated:** 2026-02-02
