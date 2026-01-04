const nodeExternals = require('webpack-node-externals');

module.exports = function (options, webpack) {
  return {
    ...options,
    externals: [
      nodeExternals({
        // 여기에 포함된 패키지는 "외부 라이브러리"로 취급하지 않고
        // 소스 코드처럼 Webpack이 직접 읽어서 번들링합니다.
        allowlist: [/^@pixelet\/shared/],
      }),
    ],
  };
};
