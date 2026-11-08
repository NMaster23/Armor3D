# Armor3D material texture assets

Extracted from the downloaded ambientCG 2K archives. Original ZIP files remain in Downloads.
Only Color, Roughness, Metalness (when supplied), NormalGL, and AmbientOcclusion maps were copied. Displacement maps and preview thumbnails were omitted for the initial renderer integration.

Folders:
- Concrete049
- DiamondPlate005B
- Fabric004
- Metal050A
- Metal063
- Plastic007
- Rubber001
- Wood051

Map conventions:
- Color = base/albedo color (sRGB)
- Roughness, Metalness, NormalGL, AmbientOcclusion = data maps (linear/non-color)
- Some materials have no Metalness or AmbientOcclusion map; use scalar material defaults for those.

The renderer still needs the code-side material/texture pipeline before these maps appear in the viewport.
