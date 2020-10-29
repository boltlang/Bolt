
const webpack = require("webpack");
const path = require("path");

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
      {
        test: /\.ts$/,
        exclude: /node_modules/,
        loader: "ts-loader",
        options: { transpileOnly: true }
      },
      {
        test: /\.m?js$/,
        exclude: /node_modules/,
        loader: 'babel-loader'
      },
    ]
  }
};

