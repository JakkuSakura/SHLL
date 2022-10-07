package shll.ast

import com.typesafe.scalalogging.Logger
import shll.ast.*
import shll.ast.AstHelper.*
import shll.backends.{PrettyPrinter, ShllPrettyPrinter}
import shll.frontends.ParamUtil.*

import scala.collection.mutable

class AstWalker() {
  val logger: Logger = Logger[this.type]
  val pp: PrettyPrinter = ShllPrettyPrinter(newlines = false)

  def walk(n: AST): AST = {
    walkNode(n)
  }

  def walkNode(n: AST): AST = {
    //    logger.debug("Eliminating " + pp.print(n))
    val x = n match {
      case n: Block => walkBlock(n)
      case n: Apply => walkApply(n)
      case n: Ident => walkIdent(n)
      case n: LiteralList => LiteralList(n.value.map(walkNode))
      case n: Field => walkField(n)
      case n: Param => walkParam(n)
      case n: Select => walkSelect(n)
      case n: Cond => walkCond(n)
      case n: ForEach => walkForEach(n)
      case n: ApplyType => walkTypeApply(n)
      case n: DefType => walkDefType(n)
      case n: DefVal => walkDefVal(n)
      case n: DefFun => walkDefFun(n)
      case n: Assign => walkAssign(n)
      case n: ApplyFun => walkFunApply(n)
      case n: Params => Params(n.params.map(walkNode(_).asInstanceOf[Param]))
      case n => n
    }
//    val orig = pp.print(n)
//    val res = pp.print(x)
//    if (orig != res)
//      logger.debug("Eliminated " + orig + " => " + res)
    x
  }

  def walkField(n: Field): Field = {
    val value = walkNode(n.ty)
    Field(n.name, value)
  }

  def walkParam(n: Param): Param = {
    val value = walkNode(n.ty)
    Param(n.name, value)
  }
  def walkDefVal(
      n: DefVal
  ): DefVal = {
    DefVal(n.name, walkNode(n.value))
  }

  def walkIdent(id: Ident): AST = {
    id
  }

  def walkApply(n: Apply): AST = {
    n
  }

  def walkTypeApply(n: ApplyType): AST = {
    n
  }

  def walkDefType(n: DefType): AST = {
    n
  }

  def walkSelect(n: Select): AST = {
    val obj = walkNode(n.obj)
    Select(obj, n.field)
  }

  def walkCond(n: Cond): AST = {
    val cond = walkNode(n.cond)
    val conseq = walkNode(n.consequence)
    val alt = walkNode(n.alternative)
    val condTotal = Cond(cond, conseq, alt)
    condTotal
  }

  def walkForEach(n: ForEach): AST = {
    val iterable = walkNode(n.iterable)
    val body = walkNode(n.body)
    val f = ForEach(n.variable, iterable, body)
    f
  }

  def walkDefFun(fun: DefFun): DefFun = {
    DefFun(
      fun.name,
      walkNode(fun.params).asInstanceOf,
      walkNode(fun.ret),
      walkNode(fun.body)
    )
  }

  def walkAssign(n: Assign): AST = {
    val value = walkNode(n.value)
    val ass = Assign(n.target, value)
    ass
  }

  def walkFunApply(n: ApplyFun): AST = {
    val args = walkNode(n.params).asInstanceOf[Params]
    val ret = walkNode(n.ret)
    val body = walkNode(n.body)
    val newApply = ApplyFun(args, ret, body)
    newApply
  }
  def walkBlock(d: Block): AST = {
    Block(
      d.children.map(walkNode)
    )
  }

}
