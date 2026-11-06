from fpdf import FPDF
pdf=FPDF();pdf.set_font("helvetica",size=12)
caps=[("1. Objetivo",1,"El sistema valida cargas de clientes."),("2. Alcance",1,"Aplica a cargas diarias."),("3. Validaciones",1,"Reglas de campos."),("3.1 Campos",2,"El campo ID es obligatorio y numerico.\nEl campo NOMBRE tiene maximo 40 caracteres."),("3.2 Errores",2,"E001: ID vacio\nE002: NOMBRE muy largo"),("4. Flujo",1,"Recepcion, validacion, carga.")]
for t,l,b in caps:
    pdf.add_page();pdf.start_section(t,level=l-1);pdf.cell(0,10,t,new_x="LMARGIN",new_y="NEXT");pdf.multi_cell(0,8,b)
pdf.add_page();pdf.multi_cell(0,8,"Pagina extra del flujo sin marcador.")
pdf.output("diseno.pdf")
