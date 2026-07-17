const path = require('path');
const HtmlWebpackPlugin = require('html-webpack-plugin');
const MiniCssExtractPlugin = require("mini-css-extract-plugin");
const WasmPackPlugin = require('@wasm-tool/wasm-pack-plugin');

module.exports = {
  entry: "./index.js",
  output: {
    path: path.resolve(__dirname, 'dist'),
    filename: "bundle.js",
  },
  mode: "development",

  experiments: {
    asyncWebAssembly: true,
  },

  module: {
    rules: [
      { test: /\.css$/, use: [ MiniCssExtractPlugin.loader, "css-loader" ] },
      { test: /\.png$/, type: "asset/resource" },
      { test: /\.html$/, loader: "html-loader" },
    ],
  },

  plugins: [
    new HtmlWebpackPlugin({
      filename: "index.html",
      favicon:  "../../doc/favicon.png",
      template: "template.html",
    }),
    new MiniCssExtractPlugin({
      filename: "styles.css",
    }),
    new WasmPackPlugin({
      crateDirectory: path.resolve(__dirname, '..'),
    }),
  ],
};
