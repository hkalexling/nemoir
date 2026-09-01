"""Feature recipes — preimplemented, deterministic, safe (no label leakage).

Each recipe is a pure function of the input features. Recipes do not see
labels. Recipe names are referenced by candidate.json's ``feature_recipe``
field and validated by config.py's schema.

Recipes:
  raw_v1      — pass through the 54 Covertype features unchanged.
  terrain_v1  — 69 features: raw + 15 derived terrain interactions
                (elevation×slope, log elevation, aspect sin/cos, slope
                categories, hillshade mean/range, distance ratios).
  minimal_v1  — 10 features: only the continuous cartographic variables
                (elevation, aspect, slope, hydrology distances, roadways,
                hillshade, fire points). Drops all 44 binary wilderness/soil
                one-hot columns. Tests whether continuous features alone
                suffice or categorical soil/wilderness info is essential.
"""

from __future__ import annotations

from typing import Any

# ── Recipe registry ──────────────────────────────────────────────────────────

_RECIPES: dict[str, Any] = {}


def _register(name: str):
    """Decorator to register a feature recipe by name."""
    def decorator(fn):
        _RECIPES[name] = fn
        return fn
    return decorator


def get_recipe(name: str):
    """Return a feature recipe function by name, or raise KeyError."""
    if name not in _RECIPES:
        raise KeyError(
            f"unknown feature recipe: {name!r}. "
            f"Available: {sorted(_RECIPES.keys())}"
        )
    return _RECIPES[name]


def list_recipes() -> list[str]:
    """Return sorted list of available recipe names."""
    return sorted(_RECIPES.keys())


# ── Recipe implementations ───────────────────────────────────────────────────


@_register("raw_v1")
def raw_v1(X: Any) -> Any:
    """Pass-through: return X unchanged as a float64 array.

    This is the simplest baseline recipe. It uses all 54 original features
    without any transformation beyond dtype safety.
    """
    import numpy as np

    X_arr = np.asarray(X, dtype=np.float64)
    if X_arr.ndim != 2 or X_arr.shape[1] != 54:
        raise ValueError(f"raw_v1 expects an (n, 54) feature matrix, got {X_arr.shape}")
    return X_arr


@_register("minimal_v1")
def minimal_v1(X: Any) -> Any:
    """Select only the 10 continuous cartographic features (columns 0-9).

    Drops all 44 binary one-hot columns (4 wilderness areas + 40 soil types).
    This tests whether continuous terrain measurements alone are sufficient
    or whether categorical soil/wilderness information is essential.
    """
    import numpy as np

    X_arr = np.asarray(X, dtype=np.float64)
    if X_arr.ndim != 2 or X_arr.shape[1] != 54:
        raise ValueError(f"minimal_v1 expects an (n, 54) feature matrix, got {X_arr.shape}")
    return X_arr[:, :10]


@_register("terrain_v1")
def terrain_v1(X: Any) -> Any:
    """Deterministic terrain-derived feature engineering for Covertype.

    Covertype features (0-indexed columns):
       0: Elevation (m)
       1: Aspect (degrees)
       2: Slope (degrees)
       3: Horizontal_Distance_To_Hydrology (m)
       4: Vertical_Distance_To_Hydrology (m)
       5: Horizontal_Distance_To_Roadways (m)
       6: Hillshade_9am (0-255)
       7: Hillshade_Noon (0-255)
       8: Hillshade_3pm (0-255)
       9: Horizontal_Distance_To_Fire_Points (m)
      10-13: Wilderness_Area (one-hot, 4 columns)
      14-53: Soil_Type (one-hot, 40 columns)

    Derived features (added after the original 54):
      - Elevation × Slope (interaction)
      - Log Elevation
      - Aspect sin / cos (circular encoding)
      - Slope categories (binned: low/med/high/steep as one-hot)
      - Hillshade mean (across 9am/noon/3pm)
      - Hillshade range (max - min)
      - Horizontal distance ratios (hydro/road, hydro/fire, road/fire)
      - Elevation × Hillshade mean
      - Slope × Hillshade mean

    Total output features: 54 original + 15 derived = 69.
    """
    import numpy as np

    X_arr = np.asarray(X, dtype=np.float64)
    if X_arr.ndim != 2 or X_arr.shape[1] != 54:
        raise ValueError(f"terrain_v1 expects an (n, 54) feature matrix, got {X_arr.shape}")

    # Extract key columns
    elev = X_arr[:, 0]
    aspect = X_arr[:, 1]
    slope = X_arr[:, 2]
    h_dist_hydro = X_arr[:, 3]
    h_dist_road = X_arr[:, 5]
    hs_9am = X_arr[:, 6]
    hs_noon = X_arr[:, 7]
    hs_3pm = X_arr[:, 8]
    h_dist_fire = X_arr[:, 9]

    derived: list[np.ndarray] = []

    # 1. Elevation × Slope interaction
    derived.append((elev * slope).reshape(-1, 1))

    # 2. Log Elevation (clip at 1 to avoid log(0))
    elev_clipped = np.clip(elev, 1.0, None)
    derived.append(np.log(elev_clipped).reshape(-1, 1))

    # 3-4. Aspect sin/cos (circular encoding for degrees)
    aspect_rad = np.deg2rad(aspect)
    derived.append(np.sin(aspect_rad).reshape(-1, 1))
    derived.append(np.cos(aspect_rad).reshape(-1, 1))

    # 5-8. Slope categories (one-hot: low [0-10), med [10-25), high [25-40), steep [40+])
    slope_low = ((slope >= 0) & (slope < 10)).astype(np.float64).reshape(-1, 1)
    slope_med = ((slope >= 10) & (slope < 25)).astype(np.float64).reshape(-1, 1)
    slope_high = ((slope >= 25) & (slope < 40)).astype(np.float64).reshape(-1, 1)
    slope_steep = (slope >= 40).astype(np.float64).reshape(-1, 1)
    derived.extend([slope_low, slope_med, slope_high, slope_steep])

    # 9. Hillshade mean
    hs_mean = ((hs_9am + hs_noon + hs_3pm) / 3.0).reshape(-1, 1)
    derived.append(hs_mean)

    # 10. Hillshade range
    hs_max = np.maximum(np.maximum(hs_9am, hs_noon), hs_3pm)
    hs_min = np.minimum(np.minimum(hs_9am, hs_noon), hs_3pm)
    derived.append((hs_max - hs_min).reshape(-1, 1))

    # 11-13. Distance ratios (clipped to avoid division by zero)
    eps_val = 1.0
    derived.append((h_dist_hydro / np.clip(h_dist_road, eps_val, None)).reshape(-1, 1))
    derived.append((h_dist_hydro / np.clip(h_dist_fire, eps_val, None)).reshape(-1, 1))
    derived.append((h_dist_road / np.clip(h_dist_fire, eps_val, None)).reshape(-1, 1))

    # 14. Elevation × Hillshade mean
    derived.append((elev * hs_mean.flatten()).reshape(-1, 1))

    # 15. Slope × Hillshade mean
    derived.append((slope * hs_mean.flatten()).reshape(-1, 1))

    derived_arr = np.hstack(derived)
    return np.hstack([X_arr, derived_arr])


def feature_count(recipe_name: str) -> int:
    """Return the number of output features for a recipe.

    Useful for schema validation without running the full recipe.
    """
    counts: dict[str, int] = {
        "raw_v1": 54,
        "terrain_v1": 69,
        "minimal_v1": 10,
    }
    return counts.get(recipe_name, -1)
