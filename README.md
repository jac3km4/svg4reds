## svg4reds
Native SVG support for redscript.

### dependencies
- [redscript](https://github.com/jac3km4/redscript)
- [RED4ext](https://github.com/WopsS/RED4ext)

### usage
- Set up an SVG file (e.g. `Cyberpunk 2077\r6\svg\example.svg`)
  ```svg
  <svg viewBox="0 0 200 100" xmlns="http://www.w3.org/2000/svg">
    <polygon points="0,100 50,25 50,75 100,0" stroke="white" stroke-width="2" />
  </svg>
  ```
- Load it in a script (e.g. `Cyberpunk 2077\r6\scripts\MyMod\Mod.reds`)
  ```swift
  import Svg.Core.*

  @wrapMethod(LoadingScreenProgressBarController)
  protected cb func OnInitialize() -> Bool {
      wrappedMethod();
      let svg = LoadSvg("example");
      svg.SetAnchor(inkEAnchor.TopRight);
      svg.SetMargin(0, 200, 200, 0);
      this.GetRootCompoundWidget().AddChildWidget(svg);
  }
  ```

### SVG support
Normal SVG shapes, paths (including bezier curves), polygons should work but this library is alpha-quality and there are many limitations:
- no animations
- no embedded images 
- no masks
- no gradients (seems relatively easy to add)
- no clip paths (seems relatively easy to add)
- fill seems to be broken for complicated shapes (WIP)
