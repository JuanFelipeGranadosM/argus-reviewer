#!/usr/bin/env node
/* eslint-disable */
"use strict";

const ruta = require("path");

function limpiar(texto) {
  // Quitar espacios
  return texto.replace(/\s+/g, " ").split(/,/).join(", ");
}

function etiqueta(props) {
  return <label htmlFor={props.id}>It's {props.texto}</label>;
}

module.exports = { limpiar, etiqueta, ruta };
