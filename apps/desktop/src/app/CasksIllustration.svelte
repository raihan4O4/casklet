<script lang="ts">
  import stack from "../assets/casks-stack.png";
  // Keep the supplied panels opaque. Only the surrounding screenshot shadow
  // uses luminance-to-alpha and edge fading; panel contours are never faded.
</script>

<svg class="casks-stack" viewBox="0 0 280 254" aria-hidden="true">
  <defs>
    <clipPath id="casks-artwork-panels">
      <path
        d="M37 111 Q37 98 49 89 L173 9 Q190 1 191 20 L191 34 L202 28 Q220 20 222 40 L222 52 L234 47 Q253 38 255 57 L255 154 Q255 165 244 173 L134 240 Q114 254 112 234 L112 205 L92 218 Q74 228 74 207 L74 181 L54 194 Q37 204 37 181 Z"
      />
    </clipPath>
    <filter id="casks-dark-material" color-interpolation-filters="sRGB">
      <feColorMatrix type="saturate" values="0" />
      <feComponentTransfer>
        <feFuncR type="linear" slope="-.56" intercept=".765" />
        <feFuncG type="linear" slope="-.572" intercept=".796" />
        <feFuncB type="linear" slope="-.588" intercept=".843" />
      </feComponentTransfer>
    </filter>
    <filter id="casks-artwork-alpha" color-interpolation-filters="sRGB">
      <feColorMatrix
        in="SourceGraphic"
        type="matrix"
        values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  -0.2126 -0.7152 -0.0722 0 1"
        result="texture"
      />
      <feFlood flood-color="currentColor" result="ink" />
      <feComposite in="ink" in2="texture" operator="in" />
    </filter>
    <linearGradient id="casks-artwork-vertical" x2="0" y2="1">
      <stop stop-color="white" stop-opacity="0" />
      <stop offset=".04" stop-color="white" stop-opacity=".45" />
      <stop offset=".12" stop-color="white" />
      <stop offset=".80" stop-color="white" />
      <stop offset=".86" stop-color="white" stop-opacity=".85" />
      <stop offset=".92" stop-color="white" stop-opacity=".4" />
      <stop offset=".97" stop-color="white" stop-opacity=".08" />
      <stop offset="1" stop-color="white" stop-opacity="0" />
    </linearGradient>
    <linearGradient id="casks-artwork-horizontal">
      <stop stop-color="white" stop-opacity="0" />
      <stop offset=".06" stop-color="white" stop-opacity=".35" />
      <stop offset=".16" stop-color="white" />
      <stop offset=".82" stop-color="white" />
      <stop offset=".94" stop-color="white" stop-opacity=".35" />
      <stop offset="1" stop-color="white" stop-opacity="0" />
    </linearGradient>
    <mask id="casks-artwork-bottom">
      <rect width="280" height="254" fill="url(#casks-artwork-vertical)" />
    </mask>
    <mask id="casks-artwork-sides">
      <rect width="280" height="254" fill="url(#casks-artwork-horizontal)" />
    </mask>
  </defs>
  <g mask="url(#casks-artwork-bottom)">
    <image
      href={stack}
      width="280"
      height="254"
      filter="url(#casks-artwork-alpha)"
      mask="url(#casks-artwork-sides)"
    />
  </g>
  <image
    class="casks-panels"
    href={stack}
    width="280"
    height="254"
    clip-path="url(#casks-artwork-panels)"
  />
</svg>
