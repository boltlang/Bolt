
const fs = require('fs');
const webpack = require("webpack");
const path = require("path");

function zip(entries) {
  const obj = {};
  for (const [k, v] of entries) {
    obj[k] = v;
  }
  return obj;
}

module.exports = {
  target: 'node',
  mode: 'development',
  entry: {
    'bolt': './src/bin/bolt.ts',
    'bolt-test': './src/bin/bolt-test.ts',
  },
  output: {
    filename: 'bin/[name].js',
    path: path.resolve(__dirname, 'build'),
    devtoolModuleFilenameTemplate: '[absolute-resource-path]'
  },
  resolve: {
    extensions: [".ts", ".js"],
  },
  externals: zip(fs.readdirSync('node_modules').map(m => [m, `commonjs ${m}`])),
  plugins: [
    new webpack.BannerPlugin({ banner: '#!/usr/bin/env node', raw: true }),
  ],
  devtool: 'source-map',
  module: {
    rules: [
      { test: /\.ts$/, exclude: /node_modules/, loader: "ts-loader", options: { transpileOnly: true } },
      { test: /\.m?js$/, exclude: /node_modules/, loader: 'babel-loader' },
    ]
  }
};

