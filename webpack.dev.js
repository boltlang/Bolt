
const webpack = require("webpack");
const path = require("path");

module.exports = {
  target: 'node',
  mode: 'development',
  entry: './src/bin/bolt.ts',
  output: {
    filename: 'bin/bolt.js',
    path: path.resolve(__dirname, 'build'),
  },
  resolve: {
    extensions: [".ts", ".js"],
  },
  plugins: [
    new webpack.BannerPlugin({ banner: '#!/usr/bin/env node', raw: true }),
  ],
  devtool: 'source-map',
  module: {
    rules: [
      { test: /\.ts$/, loader: "ts-loader", options: { transpileOnly: true } },
      { test: /\.m?js$/, exclude: /node_modules/, loader: 'babel-loader' },
    ]
  }
};

